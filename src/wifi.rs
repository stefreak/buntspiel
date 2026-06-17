//! # WiFi Connection Management
//!
//! Manages WiFi connectivity and automatic reconnection for the Buntspiel companion cube,
//! including visual feedback during connection attempts.
//!
//! ## Strategy
//! - **Primary**: Configured WiFi network.
//! - **Fallback**: Pixelblaze access point (192.168.4.1).
//! - **Recovery**: Automatic reconnection with backoff.
//!
//! ## Power Management
//! Uses CYW43 power saving mode to extend battery life.

use cyw43::Control;
use cyw43_pio::PioSpi;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_rp::{
    gpio::Output,
    peripherals::{DMA_CH0, PIN_23, PIN_24, PIN_25, PIN_29, PIO0},
    pio::{InterruptHandler, Pio},
};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

use crate::animate::wait_animation;

// WiFi Network Configuration
// TODO: Move to external config file or environment variables for security
const WIFI_NETWORK: &str = "Testturm2"; // Festival network or local WiFi
const WIFI_PASSWORD: &str = "12345678"; // TODO: Use secure credential storage

/// Maximum concurrent TCP sockets allowed (limited by Pico W RAM).
const MAX_SOCKETS: usize = 3;

/// Initialize WiFi subsystem and network stack.
pub(crate) async fn init_wifi<'a>(
    spawner: Spawner,
    spi: PioSpi<'static, PIO0, 0, DMA_CH0>,
    pwr: Output<'static>,
    random_seed: u64,
) -> &'static embassy_net::Stack<cyw43::NetDriver<'static>> {
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());

    // Load WiFi firmware and regulatory data from embedded binaries
    // These files are licensed from Infineon and included in the firmware directory
    let fw = include_bytes!("../cyw43-firmware/43439A0.bin"); // Main WiFi firmware
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin"); // Country/regulatory data

    // Initialize CYW43 WiFi driver with hardware interfaces
    info!("wifi: 🔧 Initializing CYW43 WiFi chip...");
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    // Spawn the low-level WiFi driver task (handles SPI communication)
    unwrap!(spawner.spawn(wifi_task(runner)));

    // Initialize WiFi chip with regulatory data and power management
    info!("wifi: 📡 Loading regulatory data and configuring power management...");
    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave) // Enable power saving for battery life
        .await;

    // Network configuration - use DHCP for automatic IP assignment
    let config = embassy_net::Config::dhcpv4(Default::default());

    // Alternative: Static IP configuration (uncomment if needed)
    // Useful when connecting directly to Pixelblaze access point
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 2), 24),  // Client IP
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(192, 168, 4, 1)),              // Pixelblaze IP
    //});

    // Initialize TCP/IP network stack with static memory allocation
    info!("wifi: 🌐 Creating network stack...");
    static STACK: StaticCell<embassy_net::Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<embassy_net::StackResources<MAX_SOCKETS>> = StaticCell::new();
    let stack = &*STACK.init(embassy_net::Stack::new(
        net_device,                                                        // CYW43 network device
        config,                                                            // DHCP or static config
        RESOURCES.init(embassy_net::StackResources::<MAX_SOCKETS>::new()), // Socket resources
        random_seed, // For TCP sequence numbers
    ));

    // Spawn network stack processing task (handles TCP/IP protocols)
    unwrap!(spawner.spawn(net_task(stack)));

    // Spawn connection management task (handles WiFi association and reconnection)
    unwrap!(spawner.spawn(control_task(control, stack)));

    info!("wifi: ✅ Network stack initialized successfully");
    stack
}

/// Attempt to connect to the configured WiFi network.
async fn connect_to_wifi<'d>(
    control: &'d mut Control<'static>,
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
) {
    info!("wifi: 🔍 Scanning for network '{}'...", WIFI_NETWORK);

    // Attempt WPA2 connection with configured credentials
    match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
        Ok(_) => {
            info!("wifi: 🤝 Successfully associated with network");
        }
        Err(err) => {
            info!("wifi: ❌ Association failed with status={}", err.status);
            return;
        }
    }

    // Wait for DHCP IP address assignment (not needed for static IP)
    info!("wifi: 📄 Waiting for DHCP configuration...");
    while !stack.is_config_up() {
        Timer::after_millis(500).await;
    }
    info!("wifi: ✅ Network connection established with IP configuration!");
}

/// Low-level WiFi driver task handling SPI communication with the CYW43 chip.
#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    info!("wifi: 🏃 Starting WiFi driver task");
    runner.run().await
}

/// Network stack processing task (TCP/IP).
#[embassy_executor::task]
async fn net_task(stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>) -> ! {
    info!("wifi: 📦 Starting network stack task");
    stack.run().await
}

/// WiFi connection management and monitoring task.
#[embassy_executor::task]
async fn control_task(
    mut control: Control<'static>,
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
) -> ! {
    info!("wifi: 👁️ Starting connection management task");

    loop {
        // If we're already connected, just monitor the connection
        if stack.is_config_up() {
            Timer::after(Duration::from_secs(2)).await; // Check every 2 seconds
            continue;
        }

        info!("wifi: 🔄 Network disconnected, starting reconnection sequence");

        // Run connection attempts with visual feedback
        // The select! ensures animation stops immediately when connection succeeds
        select(
            // Connection attempt loop
            async {
                while !stack.is_config_up() {
                    connect_to_wifi(&mut control, stack).await;
                    if !stack.is_config_up() {
                        info!("wifi: ⏳ Connection failed, retrying in 2 seconds...");
                        Timer::after(Duration::from_secs(2)).await;
                    }
                }
                info!("wifi: 🎉 Connection restored!");
            },
            // Visual feedback during connection attempts
            wait_animation(),
        )
        .await;
    }
}
