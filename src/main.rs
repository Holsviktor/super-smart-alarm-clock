use rumqttc::{Client, Event, MqttOptions, QoS};
use std::time::Duration;

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

    //let _ = thread::Builder::new().name("connection-iterator".into()).spawn(move || { loop {connection.iter().next();}});

    loop {
        if let Some(Ok(event)) = connection.iter().next() {
            match event {
                Event::Incoming(message) => println!("Received: {:?}", message),
                e => {
                    dbg!(&e);
                }
            }
        } else {
            println!("Err!");
        }
    }
}
