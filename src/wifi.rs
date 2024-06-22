use cyw43::Control;
use cyw43_pio::PioSpi;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_rp::{
    gpio::Output,
    peripherals::{DMA_CH0, PIO0},
};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

use crate::animate::wait_animation;

const WIFI_NETWORK: &str = "Testturm2";
const WIFI_PASSWORD: &str = "12345678";

const MAX_SOCKETS: usize = 3;

pub(crate) async fn init_wifi<'a>(
    spawner: Spawner,
    spi: PioSpi<'static, PIO0, 0, DMA_CH0>,
    pwr: Output<'static>,
    random_seed: u64,
) -> &'static embassy_net::Stack<cyw43::NetDriver<'static>> {
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

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

    // Init network stack
    static STACK: StaticCell<embassy_net::Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<embassy_net::StackResources<MAX_SOCKETS>> = StaticCell::new();
    let stack = &*STACK.init(embassy_net::Stack::new(
        net_device,
        config,
        RESOURCES.init(embassy_net::StackResources::<MAX_SOCKETS>::new()),
        random_seed,
    ));

    unwrap!(spawner.spawn(net_task(stack)));

    unwrap!(spawner.spawn(control_task(control, stack)));

    stack
}

async fn connect_to_wifi<'d>(
    control: &'d mut Control<'static>,
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
) {
    info!("Joining network {}...", WIFI_NETWORK);
    match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
        Ok(_) => {
            info!("join successful");
        }
        Err(err) => {
            info!("join failed with status={}", err.status);
            return;
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(500).await;
    }
    info!("DHCP is now up!");
}

#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn control_task(
    mut control: Control<'static>,
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
) -> ! {
    loop {
        if stack.is_config_up() {
            Timer::after(Duration::from_secs(2)).await;
            continue;
        }

        // play wait animation while connecting to a wifi
        select(
            async {
                while !stack.is_config_up() {
                    connect_to_wifi(&mut control, stack).await;
                    Timer::after(Duration::from_secs(2)).await;
                }
            },
            wait_animation(),
        )
        .await;
    }
}
