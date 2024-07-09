#![no_std]
#![no_main]

use defmt::*;
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_net::{tcp::TcpSocket, Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_stm32::{bind_interrupts, eth, gpio::Input, time::Hertz};
use embassy_stm32::{
    eth::{generic_smi::GenericSMI, Ethernet, PacketQueue},
    gpio::Pull,
};
use embassy_stm32::{gpio::AnyPin, peripherals::ETH};
use embassy_stm32::{gpio::Pin, Config};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

// MQTT related imports
use heapless::String;
use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    utils::rng_generator::CountingRng,
};
use serde::*;
use typenum::{consts::*, uint};

mod hyped_core;
use hyped_core::{
    format_string,
    logger::{LogLevel, LogTarget, Logger},
    mqtt,
};

use hyped_core::{
    mqtt::{initialise_mqtt_config, HypedMqttClient},
    mqtt_topics::MqttTopics,
};

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
});

#[derive(Serialize, Deserialize)]
struct MQTTMessage {
    topic: String<48>,
    task_id: u8,
}

#[derive(Serialize, Deserialize)]
struct ButtonMqttMessage {
    header: MQTTMessage,
    status: bool,
}

// Define a tuple type for topic, payload
struct MqttMessage {
    topic: String<48>,
    payload: String<512>,
}

static SEND_CHANNEL: Channel<ThreadModeRawMutex, ButtonMqttMessage, 64> = Channel::new();
static RECV_CHANNEL: Channel<ThreadModeRawMutex, ButtonMqttMessage, 64> = Channel::new();

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<Ethernet<'static, ETH, GenericSMI>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn button_task(pin: AnyPin) {
    let button: Input<_> = Input::new(pin, Pull::Down);
    loop {
        SEND_CHANNEL
            .send(ButtonMqttMessage {
                header: MQTTMessage {
                    topic: MqttTopics::to_string(&MqttTopics::Acceleration),
                    task_id: 0,
                },
                status: button.is_high(),
            })
            .await;
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn five_seconds_task() {
    loop {
        SEND_CHANNEL
            .send(ButtonMqttMessage {
                header: MQTTMessage {
                    topic: MqttTopics::to_string(&MqttTopics::Acceleration),
                    task_id: 2,
                },
                status: false,
            })
            .await;
        Timer::after(Duration::from_secs(5)).await;
    }
}

#[embassy_executor::task]
async fn mqtt_task(mut socket: TcpSocket<'static>) {
    // logger.log(LogLevel::Info, "Connecting...");
    info!("Connecting...");
    match socket
        .connect((Ipv4Address::new(169, 254, 195, 141), 1883))
        .await
    {
        Ok(()) => {
            // logger.log(LogLevel::Info, "Connected!"),
            info!("Connected!")
        }
        Err(connection_error) => {
            //logger.log(
            // LogLevel::Error,
            // format_string::show(
            //     &mut log_buffer,
            //     format_args!("Error connecting: {:?}", connection_error),
            // )
            // .unwrap(),
            info!("Error connecting: {:?}", connection_error)
        }
    };

    let config = initialise_mqtt_config();
    let mut recv_buffer = [0; 1024];
    let mut write_buffer = [0; 1024];
    let client = MqttClient::<_, 5, _>::new(
        socket,
        &mut write_buffer,
        1024,
        &mut recv_buffer,
        1024,
        config,
    );
    let mut mqtt_client = HypedMqttClient { client };

    match mqtt_client.connect_to_broker().await {
        Ok(()) => {
            // logger.log(LogLevel::Info, "Connected!"),
            info!("Connected!")
        }
        Err(connection_error) => {
            //logger.log(
            // LogLevel::Error,
            // format_string::show(
            //     &mut log_buffer,
            //     format_args!("Error connecting: {:?}", connection_error),
            // )
            // .unwrap(),
            info!("Error connecting: {:?}", connection_error)
        }
    }

    loop {
        while !SEND_CHANNEL.is_empty() {
            let message = SEND_CHANNEL.receive().await;
            let serialized_message =
                serde_json_core::to_string::<U512, ButtonMqttMessage>(&message).unwrap();

            mqtt_client
                .send_message(
                    message.header.topic.as_str(),
                    serialized_message.as_bytes(),
                    true,
                )
                .await;
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn mqtt_recv_task(mut socket: TcpSocket<'static>) {
    info!("Connecting...");
    match socket
        .connect((Ipv4Address::new(169, 254, 195, 141), 1883))
        .await
    {
        Ok(()) => {
            // logger.log(LogLevel::Info, "Connected!"),
            info!("Connected!")
        }
        Err(connection_error) => {
            //logger.log(
            // LogLevel::Error,
            // format_string::show(
            //     &mut log_buffer,
            //     format_args!("Error connecting: {:?}", connection_error),
            // )
            // .unwrap(),
            info!("Error connecting: {:?}", connection_error)
        }
    };
    let mut config = ClientConfig::new(
        rust_mqtt::client::client_config::MqttVersion::MQTTv5,
        CountingRng(10000),
    );
    config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);
    config.max_packet_size = 100;
    config.add_client_id("receiver-stm-client");
    let mut recv_buffer = [0; 1024];
    let mut write_buffer = [0; 1024];
    let client = MqttClient::<_, 5, _>::new(
        socket,
        &mut write_buffer,
        1024,
        &mut recv_buffer,
        1024,
        config,
    );
    let mut mqtt_client = HypedMqttClient { client };
    match mqtt_client.connect_to_broker().await {
        Ok(()) => {
            // logger.log(LogLevel::Info, "Connected!"),
            info!("Connected!")
        }
        Err(connection_error) => {
            //logger.log(
            // LogLevel::Error,
            // format_string::show(
            //     &mut log_buffer,
            //     format_args!("Error connecting: {:?}", connection_error),
            // )
            // .unwrap(),
            info!("Error connecting: {:?}", connection_error)
        }
    }

    mqtt_client.subscribe("command_sender").await;
    mqtt_client.subscribe("acceleration").await;

    loop {
        match mqtt_client.receive_message().await {
            Ok((topic, message)) => {
                info!("Received message on topic {}: {}", topic, message);
            }
            Err(err) => {
                info!("Error receiving message: {:?}", err);
            }
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let logger = Logger::new(LogLevel::Info, LogTarget::Console);
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Bypass,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL216,
            divp: Some(PllPDiv::DIV2), // 8mhz / 4 * 216 / 2 = 216Mhz
            divq: None,
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let p = embassy_stm32::init(config);
    spawner.spawn(button_task(p.PC13.degrade())).unwrap();

    logger.log(LogLevel::Info, "Hello World!");

    let seed: u64 = 0xdeadbeef;
    let mac_addr: [u8; 6] = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    static PACKETS: StaticCell<PacketQueue<4, 4>> = StaticCell::new();
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 4>::new()),
        p.ETH,
        Irqs,
        p.PA1,
        p.PA2,
        p.PC1,
        p.PA7,
        p.PC4,
        p.PC5,
        p.PG13,
        p.PB13,
        p.PG11,
        GenericSMI::new(0),
        mac_addr,
    );

    // let config = embassy_net::Config::dhcpv4(Default::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(169, 254, 195, 61), 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(Ipv4Address::new(169, 254, 195, 141)),
    });

    // Init network stack
    static STACK: StaticCell<Stack<Ethernet<'static, ETH, GenericSMI>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        device,
        config,
        RESOURCES.init(StackResources::<3>::new()),
        seed,
    ));

    // Launch network task
    unwrap!(spawner.spawn(net_task(stack)));

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    logger.log(LogLevel::Info, "Network stack initialized");
    static mut SEND_SOCKET_RX_BUFFER: [u8; 4096] = [0; 4096];
    static mut SEND_SOCKET_TX_BUFFER: [u8; 4096] = [0; 4096];
    let mut send_socket = unsafe {
        TcpSocket::new(
            &stack,
            &mut SEND_SOCKET_RX_BUFFER,
            &mut SEND_SOCKET_TX_BUFFER,
        )
    };
    send_socket.set_timeout(Some(embassy_time::Duration::from_secs(60)));
    unwrap!(spawner.spawn(mqtt_task(send_socket)));
    static mut RECV_SOCKET_RX_BUFFER: [u8; 4096] = [0; 4096];
    static mut RECV_SOCKET_TX_BUFFER: [u8; 4096] = [0; 4096];
    let mut recv_socket = unsafe {
        TcpSocket::new(
            &stack,
            &mut RECV_SOCKET_RX_BUFFER,
            &mut RECV_SOCKET_TX_BUFFER,
        )
    };
    recv_socket.set_timeout(Some(embassy_time::Duration::from_secs(60)));
    unwrap!(spawner.spawn(mqtt_recv_task(recv_socket)));
    unwrap!(spawner.spawn(five_seconds_task()));
    loop {
        SEND_CHANNEL
            .send(ButtonMqttMessage {
                header: MQTTMessage {
                    topic: MqttTopics::to_string(&MqttTopics::Acceleration),
                    task_id: 1,
                },
                status: false,
            })
            .await;
        Timer::after(Duration::from_millis(1000)).await;
    }
}
