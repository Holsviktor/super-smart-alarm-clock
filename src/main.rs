use core::str;
use rumqttc::{Client, Event, MqttOptions, Packet, Publish, QoS};
use socket2::{Domain, Protocol, Type};
use std::{
    io::Read, net::SocketAddr, sync::mpsc::{Receiver, Sender}, time::Duration,
};
use chrono::Timelike;

use light_control::{
    buttons::{ButtonAction, ButtonMessage, ControllerButton},
    lights::{
        IKEA_SWITCH_TOPIC, LightCommand, LightState, PHILLIPS_SWITCH_TOPIC, SET_TOPICS, Select,
        TOPICS,
    },
};

const HEARTBEAT : [u8 ; 9] = [b'I', b' ', b's', b'u', b'f', b'f', b'e', b'r', b'.'];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        wait_for_parent_to_die();
        let program_name = std::env::args().nth(0).expect("Failed to get program name");
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

    process_pair();
}

#[derive(Debug)]
enum Command {
    LightCommand(LightCommand),
    ControlCommand(ControlCommand),
    AlarmOnCommand,
    AlarmOffCommand,
    NoCommand,
}
#[derive(Debug)]
enum ControlCommand {
    CycleLeft,
    CycleRight,
}

fn alarm_thread(command_sender: Sender<Command>, alarm_button_receiver: Receiver<Command>) {
    let delay = Duration::from_millis(400);
    let mut alarm_on: bool = false;
    loop {
        match alarm_button_receiver.try_recv() {
            Ok(Command::AlarmOnCommand) => alarm_on = true,
            Ok(Command::AlarmOffCommand) => alarm_on = false,
            _ => (),
        };

        let current_time = chrono::Local::now();        

        if current_time.hour() == 07 && current_time.minute() == 30 && current_time.second() <= 5 {
            alarm_on = true;
        }

        if alarm_on {
            let _ = command_sender.send(Command::LightCommand(LightCommand {
                target_light: Select::All,
                target_state: LightState::off(),
            }));
            std::thread::sleep(delay);
            let _ = command_sender.send(Command::LightCommand(LightCommand {
                target_light: Select::All,
                target_state: LightState::on(),
            }));
            std::thread::sleep(delay);
        }
    }
}

fn light_controller(command_receiver: Receiver<Command>) {
    let mut mqttoptions = MqttOptions::new("rust-controller-pub", "127.0.0.1", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    let mut current_light_idx: usize = 0;
    let mut current_topic = SET_TOPICS[0].to_string();

    let _connection_thread = std::thread::Builder::new()
        .name("publisher-connection-thread".to_string())
        .spawn(move || {
            loop {
                connection.iter().next();
            }
        });

    loop {
        match command_receiver.recv().unwrap_or(Command::NoCommand) {
            Command::LightCommand(light_command) => {
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
            Command::ControlCommand(control_command) => {
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
            Command::NoCommand => (),
            _ => (),
        }
    }
}

fn lightswitch_loop(command_sender: Sender<Command>, alarm_button_sender: Sender<Command>) {
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
            _ => Ok(Command::NoCommand),
        }
    }

    fn ikea_switch_callback(button_press: ButtonMessage) -> Command {
        match button_press.action {
            ButtonAction::Press(ControllerButton::On) => Command::LightCommand(LightCommand {
                target_light: Select::All,
                target_state: LightState::on(),
            }),
            ButtonAction::Press(ControllerButton::Off) => Command::LightCommand(LightCommand {
                target_light: Select::All,
                target_state: LightState::off(),
            }),
            ButtonAction::Press(ControllerButton::Left) => {
                Command::ControlCommand(ControlCommand::CycleLeft)
            }
            ButtonAction::Press(ControllerButton::Right) => {
                Command::ControlCommand(ControlCommand::CycleRight)
            }
            _ => Command::NoCommand,
        }
    }

    fn phillips_switch_callback(button_press: ButtonMessage) -> Command {
        match button_press.action {
            ButtonAction::Press(ControllerButton::On) => Command::LightCommand(LightCommand {
                target_light: Select::All,
                target_state: LightState::on(),
            }),
            ButtonAction::Press(ControllerButton::Off) => Command::LightCommand(LightCommand {
                target_light: Select::All,
                target_state: LightState::off(),
            }),
            ButtonAction::Press(ControllerButton::Up) => Command::AlarmOnCommand,
            ButtonAction::Press(ControllerButton::Down) => Command::AlarmOffCommand,
            _ => Command::NoCommand,
        }
    }
    let mut mqttoptions = MqttOptions::new("rust-controller-sub", "127.0.0.1", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    for topic in TOPICS {
        client.subscribe(topic, QoS::AtMostOnce).unwrap();
    }

    loop {
        if let Some(Ok(event)) = connection.iter().next() {
            match event {
                Event::Incoming(message) => {
                    if let Packet::Publish(publish_message) = message {
                        let command =
                            handle_button_press(publish_message).unwrap_or(Command::NoCommand);
                        let _ = match command {
                            Command::AlarmOnCommand | Command::AlarmOffCommand => {
                                alarm_button_sender.send(command)
                            }
                            _ => command_sender.send(command),
                        };
                    }
                }
                _ => (),
            }
        } else {
            eprintln!("[Subscriber] Could not advance connection!");
        }
    }
}

// Process Pair
fn wait_for_parent_to_die() {
    let port : u16 = 6767;
    let address = SocketAddr::new("127.0.0.1".parse().expect("failed to parse host IP"), port).into();

    let mut socket = socket2::Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).expect("Failed to create socket");
    let _ = socket.set_freebind_v4(true);
    let _ = socket.set_reuse_port(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(1500)));

    let _ = socket.bind(&address);
    
    loop {
        let mut buf = vec![0 ; 1024];
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
            return 
        }
    }
}

fn process_pair() -> ! {
    let port : u16 = 6767;
    let address = SocketAddr::new("127.0.0.1".parse().expect("failed to parse host IP"), port).into();

    let socket = socket2::Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).expect("Failed to create socket");
    let _ = socket.set_freebind_v4(true);
    let _ = socket.set_reuse_port(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(1500)));
    
    loop {
        let _ = socket.send_to(HEARTBEAT.as_slice(), &address);
        std::thread::sleep(Duration::from_millis(500));
    }
}
