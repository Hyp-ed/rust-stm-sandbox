use defmt::*;
use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    packet::v5::reason_codes::ReasonCode,
    utils::rng_generator::CountingRng,
};

pub struct HypedMqttClient<
    'a,
    T: embedded_io_async::Read + embedded_io_async::Write,
    R: rand_core::RngCore,
> {
    pub client: MqttClient<'a, T, 5, R>,
}

pub fn initialise_mqtt_config() -> ClientConfig<'static, 5, CountingRng> {
    let mut config = ClientConfig::new(
        rust_mqtt::client::client_config::MqttVersion::MQTTv5,
        CountingRng(20000),
    );
    config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);
    config.add_client_id("stm-client");
    config.max_packet_size = 100;

    config
}

// Implement send_message for HypedMqttClient
impl<'a, T: embedded_io_async::Read + embedded_io_async::Write, R: rand_core::RngCore>
    HypedMqttClient<'a, T, R>
{
    pub async fn connect_to_broker(&mut self) -> Result<(), ReasonCode> {
        match self.client.connect_to_broker().await {
            Ok(()) => {}
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    info!("MQTT Network Error");
                    return Err(ReasonCode::NetworkError);
                }
                _ => {
                    warn!("Other MQTT Error: {:?}", mqtt_error);
                    return Err(mqtt_error);
                }
            },
        }
        Ok(())
    }

    pub async fn send_message(&mut self, topic: &str, message: &[u8], retain: bool) {
        match self
            .client
            .send_message(
                topic,
                message,
                rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1,
                retain,
            )
            .await
        {
            Ok(()) => {}
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    info!("MQTT Network Error");
                }
                _ => {
                    warn!("Other MQTT Error: {:?}", mqtt_error);
                }
            },
        }
    }

    pub async fn subscribe(&mut self, topic: &str) {
        match self.client.subscribe_to_topic(topic).await {
            Ok(()) => {}
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    info!("MQTT Network Error");
                }
                _ => {
                    warn!("Other MQTT Error: {:?}", mqtt_error);
                }
            },
        }
    }

    pub async fn receive_message(&mut self) -> Result<&str, ReasonCode> {
        match self.client.receive_message().await {
            Ok((_topic, payload)) => {
                let payload_str = core::str::from_utf8(payload).unwrap();
                Ok(payload_str)
            }
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    info!("MQTT Network Error");
                    return Err(ReasonCode::NetworkError);
                }
                _ => {
                    warn!("Other MQTT Error: {:?}", mqtt_error);
                    return Err(mqtt_error);
                }
            },
        }
    }
}
