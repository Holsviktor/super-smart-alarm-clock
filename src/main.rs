use rumqttc::{Client, Event, MqttOptions, Packet, Publish, QoS};
use core::str;
use std::time::Duration;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ButtonAction {
    action : String,
}

const IKEA_SWITCH_TOPIC : &str = "zigbee2mqtt/ikea-bryter";
const PHILLIPS_SWITCH_TOPIC : &str = "zigbee2mqtt/phillips-bryter";
const MAIN_LIGHT_TOPIC : &str = "zigbee2mqtt/taklys";
const TOP_LIGHT_TOPIC : &str = "zigbee2mqtt/topplys";
const MID_LIGHT_TOPIC : &str = "zigbee2mqtt/midtlys";
const NIGHT_LIGHT_TOPIC : &str = "zigbee2mqtt/nattlys";

const TOPICS: [&str; 6] = [
    "zigbee2mqtt/ikea-bryter",
    "zigbee2mqtt/phillips-bryter",
    "zigbee2mqtt/taklys",
    "zigbee2mqtt/topplys",
    "zigbee2mqtt/midtlys",
    "zigbee2mqtt/nattlys",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mqttoptions = MqttOptions::new("rumqtt-sync", "127.0.0.1", 1883);
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
                        parse_message(publish_message);
                    }
                }
                _ => (),
            }
        } else {
            println!("Err!");
        }
    }
}

fn parse_message(message : Publish) -> Result<(), Box<dyn std::error::Error>> {
    let payload_string : &str = str::from_utf8(&message.payload)?;
    match message.topic.as_str() {
        IKEA_SWITCH_TOPIC => {
                if let Ok(button_press) = serde_json::from_str(payload_string) {
                    ikea_switch_callback(button_press);
                }
        },
        PHILLIPS_SWITCH_TOPIC => (),
        MAIN_LIGHT_TOPIC => (),
        TOP_LIGHT_TOPIC => (),
        MID_LIGHT_TOPIC => (),
        NIGHT_LIGHT_TOPIC => (),
        _ => (),
    }

    Ok(())
}

fn ikea_switch_callback(button_press : ButtonAction) {
    match button_press.action.as_str() {
        "on" => println!("on"),
        "off" => println!("off"),
        "arrow_left_click" => println!("left"),
        "arrow_right_click" => println!("right"),
        _ => (),
    }
}
