use chrono::Timelike;
use core::str;
use rumqttc::{Client, Event, MqttOptions, Packet, Publish, QoS};
use socket2::{Domain, Protocol, Type};
use std::{
    io::Read,
    net::SocketAddr,
    path::Path,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};
use toml::Table;

use light_control::{
    buttons::{ButtonAction, ButtonMessage, ControllerButton},
    lights::{
        LightCommand, LightState, Select, IKEA_SWITCH_TOPIC, PHILLIPS_SWITCH_TOPIC, SET_TOPICS,
        TOPICS,
    },
};

const HEARTBEAT: [u8; 9] = [b'I', b' ', b's', b'u', b'f', b'f', b'e', b'r', b'.'];

const ALARM_MIN_DURATION_SECONDS: u32 = 5;
static mut ALARM_HOUR: u32 = 0;
static mut ALARM_MINUTE: u32 = 0;
static mut ALARM_SECOND: u32 = 0;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    read_config();
    {
        wait_for_parent_to_die();
        let program_name = std::env::args().next().expect("Failed to get program name");
        let _child = std::process::Command::new(program_name).spawn();
    }

    let (command_sender, command_receiver) = std::sync::mpsc::channel::<Command>();
    let (alarm_button_sender, alarm_button_receiver) = std::sync::mpsc::channel::<Command>();

    let button_sender = command_sender.clone();
    let _button_thread = std::thread::Builder::new()
        .name("button-subscriber".to_string())
        .spawn(move || lightswitch_loop(button_sender, alarm_button_sender));

    let alarm_sender = command_sender.clone();
    let _alarm_thread = std::thread::Builder::new()
        .name("fuck-you-alarm".to_string())
        .spawn(move || alarm_thread(alarm_sender, alarm_button_receiver));

    let _light_thread = std::thread::Builder::new()
        .name("light-publisher".to_string())
        .spawn(move || light_controller(command_receiver));

    send_keepalives_to_child();
}

#[derive(Debug)]
enum Command {
    Light(LightCommand),
    Control(ControlCommand),
    AlarmOn,
    AlarmOff,
    None,
}
#[derive(Debug)]
enum ControlCommand {
    CycleLeft,
    CycleRight,
}

fn alarm_thread(command_sender: Sender<Command>, alarm_button_receiver: Receiver<Command>) {
    struct AlarmTime {
        hour: u32,
        minute: u32,
        second: u32,
    }

    let delay = Duration::from_millis(400);
    let mut alarm_on: bool = false;

    let (start_time, allow_stop_time) = unsafe {
        let second_with_margin = ALARM_SECOND + ALARM_MIN_DURATION_SECONDS;
        let minute_with_margin = ALARM_MINUTE + second_with_margin / 60;
        let hour_with_margin = ALARM_HOUR + minute_with_margin / 60;

        let start_time = AlarmTime {
            hour: ALARM_HOUR,
            minute: ALARM_MINUTE,
            second: ALARM_SECOND,
        };
        let allow_stop_time = AlarmTime {
            hour: hour_with_margin,
            minute: minute_with_margin,
            second: second_with_margin,
        };

        (start_time, allow_stop_time)
    };

    loop {
        match alarm_button_receiver.try_recv() {
            Ok(Command::AlarmOn) => alarm_on = true,
            Ok(Command::AlarmOff) => alarm_on = false,
            _ => (),
        };

        let current_time = chrono::Local::now();
        let correct_hour =
            start_time.hour <= current_time.hour() && current_time.hour() <= allow_stop_time.hour;
        let correct_minute = start_time.minute <= current_time.minute()
            && current_time.minute() <= allow_stop_time.minute;
        let correct_second = start_time.second <= current_time.second()
            && current_time.second() <= allow_stop_time.second;

        if correct_hour && correct_minute && correct_second {
            alarm_on = true;
        }

        if alarm_on {
            let _ = command_sender.send(Command::Light(LightCommand {
                target_light: Select::All,
                target_state: LightState::off(),
            }));
            std::thread::sleep(delay);
            let _ = command_sender.send(Command::Light(LightCommand {
                target_light: Select::All,
                target_state: LightState::on(),
            }));
        }
        std::thread::sleep(delay);
    }
}

fn light_controller(command_receiver: Receiver<Command>) {
    return;
    let mut mqttoptions = MqttOptions::new("rust-controller-pub", "127.0.0.1", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    let mut current_light_idx: usize = 0;
    let mut current_topic = SET_TOPICS[0].to_string();

    let _connection_thread = std::thread::Builder::new()
        .name("publisher-connection-thread".to_string())
        .spawn(move || loop {
            connection.iter().next();
        });

    loop {
        match command_receiver.recv().unwrap_or(Command::None) {
            Command::Light(light_command) => {
                let payload = light_command.as_json();

                let topics = match light_command.topics() {
                    Some(light_topics) => light_topics,
                    None => match light_command.target_light {
                        Select::Current => Box::new([current_topic.clone()]),
                        Select::None => continue,
                        _ => {
                            panic!("Did not fetch topics when some light or all lights were given")
                        }
                    },
                };

                for topic in topics {
                    let _ =
                        client.try_publish(topic.as_str(), QoS::AtMostOnce, false, payload.clone());
                }
            }
            Command::Control(control_command) => {
                let topic_count = SET_TOPICS.len();
                match control_command {
                    ControlCommand::CycleLeft => {
                        current_light_idx = (current_light_idx + topic_count - 1) % topic_count
                    }
                    ControlCommand::CycleRight => {
                        current_light_idx = (current_light_idx + topic_count + 1) % topic_count
                    }
                };

                current_topic = SET_TOPICS[current_light_idx].to_string();
            }
            Command::None => (),
            _ => (),
        }
    }
}

fn lightswitch_loop(command_sender: Sender<Command>, alarm_button_sender: Sender<Command>) {
    return;
    fn handle_button_press(message: Publish) -> Result<Command, Box<dyn std::error::Error>> {
        let payload_string: &str = str::from_utf8(&message.payload)?;
        match message.topic.as_str() {
            IKEA_SWITCH_TOPIC => {
                if let Ok(button_press) = serde_json::from_str(payload_string) {
                    Ok(ikea_switch_callback(button_press))
                } else {
                    dbg!(&payload_string);
                    Err(Box::new(std::io::Error::other("oopsie")))
                }
            }
            PHILLIPS_SWITCH_TOPIC => {
                if let Ok(button_press) = serde_json::from_str(payload_string) {
                    Ok(phillips_switch_callback(button_press))
                } else {
                    dbg!(&payload_string);
                    Err(Box::new(std::io::Error::other("oopsie")))
                }
            }
            _ => Ok(Command::None),
        }
    }

    fn ikea_switch_callback(button_press: ButtonMessage) -> Command {
        match button_press.action {
            ButtonAction::Press(ControllerButton::On) => Command::Light(LightCommand {
                target_light: Select::All,
                target_state: LightState::on(),
            }),
            ButtonAction::Press(ControllerButton::Off) => Command::Light(LightCommand {
                target_light: Select::All,
                target_state: LightState::off(),
            }),
            ButtonAction::Press(ControllerButton::Left) => {
                Command::Control(ControlCommand::CycleLeft)
            }
            ButtonAction::Press(ControllerButton::Right) => {
                Command::Control(ControlCommand::CycleRight)
            }
            _ => Command::None,
        }
    }

    fn phillips_switch_callback(button_press: ButtonMessage) -> Command {
        match button_press.action {
            ButtonAction::Press(ControllerButton::On) => Command::Light(LightCommand {
                target_light: Select::All,
                target_state: LightState::on(),
            }),
            ButtonAction::Press(ControllerButton::Off) => Command::Light(LightCommand {
                target_light: Select::All,
                target_state: LightState::off(),
            }),
            ButtonAction::Press(ControllerButton::Up) => Command::AlarmOn,
            ButtonAction::Press(ControllerButton::Down) => Command::AlarmOff,
            _ => Command::None,
        }
    }
    let mut mqttoptions = MqttOptions::new("rust-controller-sub", "127.0.0.1", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    for topic in TOPICS {
        client.subscribe(topic, QoS::AtMostOnce).unwrap();
    }

    loop {
        if let Some(Ok(Event::Incoming(message))) = connection.iter().next() {
            if let Packet::Publish(publish_message) = message {
                let command = handle_button_press(publish_message).unwrap_or(Command::None);
                let _ = match command {
                    Command::AlarmOn | Command::AlarmOff => alarm_button_sender.send(command),
                    _ => command_sender.send(command),
                };
            }
        } else {
            eprintln!("[Subscriber] Could not advance connection!");
        }
    }
}

// Process Pair
fn wait_for_parent_to_die() {
    let port: u16 = 6767;
    let address =
        SocketAddr::new("127.0.0.1".parse().expect("failed to parse host IP"), port).into();

    let mut socket = socket2::Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .expect("Failed to create socket");
    let _ = socket.set_freebind_v4(true);
    let _ = socket.set_reuse_port(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(1500)));

    let _ = socket.bind(&address);

    loop {
        let mut buf = vec![0; 1024];
        if let Ok(bytes_read) = socket.read(&mut buf) {
            buf.shrink_to(bytes_read);
            if let Ok(_heartbeat) = String::from_utf8(buf) {
                if bytes_read != 9 {
                    return;
                }
            } else {
                return;
            }
        } else {
            return;
        }
    }
}

fn send_keepalives_to_child() -> ! {
    let port: u16 = 6767;
    let address =
        SocketAddr::new("127.0.0.1".parse().expect("failed to parse host IP"), port).into();

    let socket = socket2::Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .expect("Failed to create socket");
    let _ = socket.set_freebind_v4(true);
    let _ = socket.set_reuse_port(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(1500)));

    loop {
        let _ = socket.send_to(HEARTBEAT.as_slice(), &address);
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn read_config() {
    let config = {
        let home = std::env::var("HOME").expect("Failed to fetch HOME directory from env.");
        let config_file = Path::new(&home).join(Path::new(".config/light-control.toml"));
        let config_contents =
            String::from_utf8(std::fs::read(config_file).expect("Failed to open config file."))
                .expect("Failed to parse config contents to string");
        config_contents
            .parse::<Table>()
            .expect("Failed to parse config file as toml")
    };

    let alarm_config = config
        .get("Alarm")
        .expect("Config should contain [Alarm] section/");
    unsafe {
        ALARM_HOUR = alarm_config
            .get("hour")
            .expect("Config should contain [Alarm] hour")
            .clone()
            .try_into()
            .unwrap();
        ALARM_MINUTE = alarm_config
            .get("minute")
            .expect("Config should contain [Alarm] minute")
            .clone()
            .try_into()
            .unwrap();
        ALARM_SECOND = alarm_config
            .get("second")
            .expect("Config should contain [Alarm] second")
            .clone()
            .try_into()
            .unwrap();
    }
}
