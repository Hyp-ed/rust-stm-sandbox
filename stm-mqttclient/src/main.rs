#![no_std]
#![no_main]

use defmt::*;
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_net::{tcp::TcpSocket, Ipv4Address};
use embassy_stm32::{bind_interrupts, eth};
use embassy_time::{Duration, Timer};

// MQTT related imports
use rust_mqtt::client::client::MqttClient;

mod hyped_core;
use hyped_core::format_string;
use hyped_core::logger::{LogLevel, LogTarget, Logger};
use hyped_core::network::initalise_network_stack;
use hyped_core::{
    mqtt::{initialise_mqtt_config, HypedMqttClient},
    mqtt_topics::MqttTopics,
};

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    info!("Hello World!");

    // Initialize network task
    let stack = initalise_network_stack(spawner, Irqs).await;

    // Then we can use it!
    let mut socket_rx_buffer = [0; 4096];
    let mut socket_tx_buffer = [0; 4096];

    let logger = Logger::new(LogLevel::Info, LogTarget::Console);

    loop {
        let mut socket = TcpSocket::new(&stack, &mut socket_rx_buffer, &mut socket_tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        logger.log(LogLevel::Info, "Connecting...");
        match socket
            .connect((Ipv4Address::new(169, 254, 195, 141), 1883))
            .await
        {
            Ok(()) => logger.log(LogLevel::Info, "Connected!"),
            Err(connection_error) => error!("Socket connection error: {:?}", connection_error),
        };

        let config = initialise_mqtt_config();
        let mut recv_buffer = [0; 80];
        let mut write_buffer = [0; 80];
        let client =
            MqttClient::<_, 5, _>::new(socket, &mut write_buffer, 80, &mut recv_buffer, 80, config);
        let mut mqtt_client = HypedMqttClient { client };

        match mqtt_client.connect_to_broker().await {
            Ok(()) => info!("Connected to MQTT Broker"),
            Err(mqtt_error) => error!("Error connecting to MQTT Broker: {:?}", mqtt_error),
        }

        let mut counter = 0;
        loop {
            let mut buffer = [0; 80];
            let message: &str =
                format_string::show(&mut buffer, format_args!("Counter: {}", counter)).unwrap();
            mqtt_client
                .send_message(
                    MqttTopics::to_string(&MqttTopics::Acceleration).as_str(),
                    message.as_bytes(),
                    true,
                )
                .await;
            counter += 1;
            Timer::after(Duration::from_millis(1000)).await;
        }
    }
}
