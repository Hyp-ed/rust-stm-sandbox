use async_trait::async_trait;
use colored::Colorize;
use hyped_core::{mqtt::ButtonMqttMessage, mqtt_topics::MqttTopics};
use mqrstt::{
    new_tokio,
    packets::{self, Packet},
    tokio::NetworkStatus,
    AsyncEventHandler, ConnectOptions, MqttClient,
};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::time::Duration;

pub struct PingPong {
    pub client: MqttClient,
}

#[async_trait]
impl AsyncEventHandler for PingPong {
    // Handlers only get INCOMING packets. This can change later.
    async fn handle(&mut self, event: packets::Packet) -> () {
        match event {
            Packet::Publish(p) => {
                if let Ok(payload) = String::from_utf8(p.payload.to_vec()) {
                    let message: ButtonMqttMessage = serde_json::from_str(&payload).unwrap();
                    if message.task_id == 1 {
                        println!("{}", "Ping from main event loop".yellow());
                    } else if message.task_id == 0 {
                        println!("Button pressed: {}", message.status);
                    } else if message.task_id == 2 {
                        println!("{}", "Ping from five second loop".green());
                    }
                }
            }
            Packet::ConnAck(_) => {
                println!("Connected!")
            }
            _ => (),
        }
    }
}

#[tokio::main]
async fn main() {
    let options = ConnectOptions::new("rust-client".to_string());

    let (mut network, client) = new_tokio(options);

    let stream = tokio::net::TcpStream::connect(("localhost", 1883))
        .await
        .unwrap();

    let mut pingpong = PingPong {
        client: client.clone(),
    };

    network.connect(stream, &mut pingpong).await.unwrap();

    client
        .subscribe(MqttTopics::to_string(&MqttTopics::Acceleration))
        .await
        .unwrap();

    let (n, _) = tokio::join!(
        async {
            loop {
                return match network.poll(&mut pingpong).await {
                    Ok(NetworkStatus::Active) => continue,
                    otherwise => otherwise,
                };
            }
        },
        async {
            tokio::time::sleep(Duration::from_secs(6000)).await;
            client.disconnect().await.unwrap();
        }
    );
    assert!(n.is_ok());
}
