use async_trait::async_trait;
use bytes::Bytes;
use mqrstt::{
    new_tokio,
    packets::{self, Packet},
    tokio::NetworkStatus,
    AsyncEventHandler, ConnectOptions, MqttClient,
};
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
                    if payload.to_lowercase().contains("ping") {
                        self.client
                            .publish("reply", p.qos, p.retain, Bytes::from_static(b"pong"))
                            .await
                            .unwrap();
                        println!("Received Ping, Send pong!");
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
    let mut options = ConnectOptions::new("TokioTcpPingPongExample".to_string());
    options.username = Some("user1".to_string());
    options.password = Some("admin".to_string());

    let (mut network, client) = new_tokio(options);

    let stream = tokio::net::TcpStream::connect(("localhost", 1883))
        .await
        .unwrap();

    let mut pingpong = PingPong {
        client: client.clone(),
    };

    network.connect(stream, &mut pingpong).await.unwrap();

    client.subscribe("command_handler").await.unwrap();

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
            tokio::time::sleep(Duration::from_secs(30)).await;
            client.disconnect().await.unwrap();
        }
    );
    assert!(n.is_ok());
}
