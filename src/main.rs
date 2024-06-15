#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use adafruit_seesaw::devices::{NeoTrellis, SeesawDevice, SeesawDeviceInit};
use adafruit_seesaw::prelude::{EventType, KeypadModule, NeopixelModule};
use adafruit_seesaw::SeesawRefCell;
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::{InterruptExecutor, Spawner};
use embassy_net::tcp::TcpSocket;
use embassy_net::{IpEndpoint, Ipv4Address, Stack, StackResources};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::interrupt::Interrupt;
use embassy_rp::interrupt::{InterruptExt, Priority};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, interrupt};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

// Medium priority interrupt executor for driving neotrellis display
static EXECUTOR_NEOTRELLIS: InterruptExecutor = InterruptExecutor::new();

#[interrupt]
unsafe fn SWI_IRQ_1() {
    EXECUTOR_NEOTRELLIS.on_interrupt()
}

const WIFI_NETWORK: &str = "Testturm2";
const WIFI_PASSWORD: &str = "12345678";

#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn drive_neotrellis(
    i2c: embassy_rp::i2c::I2c<'static, embassy_rp::peripherals::I2C1, embassy_rp::i2c::Blocking>,
) {
    let delay = embassy_time::Delay;
    let seesaw = SeesawRefCell::new(delay, i2c);
    let mut neotrellis = NeoTrellis::new_with_default_addr(seesaw.acquire_driver())
        .init()
        .expect("Failed to start neotrellis");

    loop {
        Timer::after(Duration::from_millis(10)).await;
        let result = neotrellis.poll();
        if result.is_err() {
            info!("neotrellis: error");
            Timer::after(Duration::from_secs(2)).await;
            continue;
        }
        for evt in result.unwrap() {
            info!(
                "neutrellis: Event: x={} y={}, {}",
                evt.x,
                evt.y,
                match evt.event {
                    EventType::Pressed => "Pressed",
                    EventType::Released => "Released",
                    _ => "Unknown",
                }
            );
            match evt.event {
                EventType::Pressed => {
                    neotrellis
                        .set_nth_neopixel_color(((evt.y * 4) + evt.x).into(), 0xf, 0xf, 0xf)
                        .unwrap();
                    neotrellis.sync_neopixel().unwrap();
                }
                EventType::Released => {
                    neotrellis
                        .set_nth_neopixel_color(((evt.y * 4) + evt.x).into(), evt.x, 0xf, evt.y)
                        .unwrap();
                    neotrellis.sync_neopixel().unwrap();
                }
                _ => {}
            };
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let p = embassy_rp::init(Default::default());

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi: PioSpi<PIO0, 0, DMA_CH0> = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = embassy_net::Config::dhcpv4(Default::default());
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 69, 2), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(192, 168, 69, 1)),
    //});

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    // Init network stack
    static STACK: StaticCell<Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::<2>::new()),
        seed,
    ));

    unwrap!(spawner.spawn(net_task(stack)));
    unwrap!(spawner.spawn(control_task(control, stack)));

    // Pixelblaze websocket
    unwrap!(spawner.spawn(pixelblaze_websocket(stack)));

    Interrupt::SWI_IRQ_1.set_priority(Priority::P3);
    let send_spawner = EXECUTOR_NEOTRELLIS.start(interrupt::SWI_IRQ_1);

    let mut config = embassy_rp::i2c::Config::default();
    config.frequency = 50_000;
    let i2c = embassy_rp::i2c::I2c::new_blocking(p.I2C1, p.PIN_7, p.PIN_6, config);
    unwrap!(send_spawner.spawn(drive_neotrellis(i2c)));
}

#[embassy_executor::task]
async fn control_task(
    mut control: cyw43::Control<'static>,
    stack: &'static Stack<cyw43::NetDriver<'static>>,
) {
    info!("Joining network {}...", WIFI_NETWORK);
    //control.join_open(WIFI_NETWORK).await;
    match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
        Ok(_) => info!("join successful"),
        Err(err) => {
            info!("join failed with status={}", err.status);
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
        control.gpio_set(0, true).await;
        Timer::after(Duration::from_millis(100)).await;
        control.gpio_set(0, false).await;
        Timer::after(Duration::from_millis(100)).await;
    }
    info!("DHCP is now up!");
}

#[embassy_executor::task]
async fn pixelblaze_websocket(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        info!("Connecting to Pixelblaze http://192.168.4.1:81...");
        if let Err(e) = socket
            .connect(IpEndpoint::new(Ipv4Address::new(192, 168, 4, 1).into(), 81))
            .await
        {
            warn!("connect error: {:?}. Trying again in 5 seconds", e);
            Timer::after(Duration::from_secs(5)).await;
            continue;
        }

        info!("Received connection from {:?}", socket.remote_endpoint());

        loop {
            match socket.write_all("hello".as_bytes()).await {
                Ok(()) => {}
                Err(e) => {
                    warn!("write error: {:?}", e);
                    break;
                }
            };

            Timer::after(Duration::from_millis(1000)).await;
        }
    }
}
