#![no_std]
#![no_main]

use defmt::*;
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_net::{tcp::TcpSocket, Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_stm32::peripherals::ETH;
use embassy_stm32::Config;
use embassy_stm32::{bind_interrupts, eth, gpio::Input, time::Hertz};
use embassy_stm32::{
    eth::{generic_smi::GenericSMI, Ethernet, PacketQueue},
    gpio::Pull,
};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

// MQTT related imports
use heapless::String;
use rust_mqtt::client::client::MqttClient;
use serde::*;
use typenum::consts::*;

mod hyped_core;
use hyped_core::logger::{LogLevel, LogTarget, Logger};

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
}

#[derive(Serialize, Deserialize)]
struct ButtonMqttMessage {
    header: MQTTMessage,
    status: bool,
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<Ethernet<'static, ETH, GenericSMI>>) -> ! {
    stack.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
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
    let button = Input::new(p.PC13, Pull::Down);

    info!("Hello World!");

    let seed: u64 = 0xdeadbeef;
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

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
    static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        device,
        config,
        RESOURCES.init(StackResources::<2>::new()),
        seed,
    ));

    // Launch network task
    unwrap!(spawner.spawn(net_task(stack)));

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    info!("Network stack initialized");

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
            Ok(()) => info!("Connected to MQTT Broker"),
            Err(mqtt_error) => error!("Error connecting to MQTT Broker: {:?}", mqtt_error),
        }

        loop {
            // let message: &str =
            // format_string::show(&mut buffer, format_args!("Counter: {}", counter)).unwrap();
            let message =
                serde_json_core::to_string::<U512, ButtonMqttMessage>(&ButtonMqttMessage {
                    header: MQTTMessage {
                        topic: MqttTopics::to_string(&MqttTopics::Acceleration),
                    },
                    status: button.is_high(),
                })
                .unwrap();
            info!("Sending message: {}", message.as_str());
            mqtt_client
                .send_message(
                    MqttTopics::to_string(&MqttTopics::Acceleration).as_str(),
                    message.as_bytes(),
                    true,
                )
                .await;
            Timer::after(Duration::from_millis(1000)).await;
        }
    }
}
