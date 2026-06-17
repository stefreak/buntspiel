//! # Buntspiel - Festival Lighthouse Companion Cube
//!
//! Main application entry point for the Buntspiel LED companion cube.
//! This system connects to a Pixelblaze lighthouse via WiFi to display
//! real-time pattern previews on a 4x4 NeoTrellis LED matrix.
//!
//! ## Hardware Configuration
//! - **Raspberry Pi Pico W**: Main microcontroller with WiFi
//! - **NeoTrellis 4x4**: RGB LED matrix connected via I2C
//!   - SDA: Pin 6 (GPIO6)
//!   - SCL: Pin 7 (GPIO7)
//!
//! ## Architecture
//! - **Core 0**: WiFi management, WebSocket communication, Pixelblaze protocol
//! - **Core 1**: I2C communication, LED matrix control, animations
//!
//! ## Network
//! - Connects to Pixelblaze at 192.168.4.1:81 via WebSocket
//! - Receives real-time LED preview frames
//! - Sends pattern control commands

#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]
#![feature(impl_trait_in_assoc_type)]

// Application modules
mod animate; // Fallback animations (spinning pattern while connecting)
mod neotrellis; // NeoTrellis 4x4 LED matrix driver and control
mod pixelblaze; // Pixelblaze WebSocket protocol and communication
mod wifi; // WiFi connection management and initialization

use cyw43_pio::PioSpi;
use defmt::{info, unwrap};
use embassy_executor::{Executor, Spawner};
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    i2c,
    multicore::spawn_core1,
    peripherals::{DMA_CH0, PIO0},
    pio::{self, Pio},
};
use neotrellis::{neotrellis_task, I2C_FREQUENCY};
use pixelblaze::pixelblaze_task;
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use static_cell::StaticCell;
use wifi::init_wifi;
use {defmt_rtt as _, panic_probe as _};

// Dual-core setup: Core 1 handles I2C and LED operations to avoid blocking WiFi
static mut CORE1_STACK: embassy_rp::multicore::Stack<4096> = embassy_rp::multicore::Stack::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();

// Interrupt binding for PIO (Programmable I/O) used by WiFi SPI communication
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("🚀 Buntspiel Companion Cube Starting Up!");

    // Initialize random number generator for WiFi and networking
    // Fixed seed for deterministic behavior during development
    let mut rng = SmallRng::from_seed(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef_u128.to_le_bytes());

    // Initialize Pico W peripherals
    let p = embassy_rp::init(Default::default());

    // WiFi chip (CYW43) control pins
    let pwr = Output::new(p.PIN_23, Level::Low); // Power control
    let cs = Output::new(p.PIN_25, Level::High); // Chip select

    // PIO-based SPI for WiFi communication (more efficient than bit-banging)
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi: PioSpi<PIO0, 0, DMA_CH0> = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,        // Chip select
        p.PIN_24,  // Clock (SCLK)
        p.PIN_29,  // Data I/O (MOSI/MISO)
        p.DMA_CH0, // DMA channel for efficient transfers
    );

    // Initialize WiFi and network stack
    info!("📡 Initializing WiFi connection...");
    let net_stack = init_wifi(spawner, spi, pwr, rng.next_u64()).await;

    // Start Pixelblaze WebSocket communication task on Core 0
    // This handles the real-time pattern data streaming
    info!("🌐 Starting Pixelblaze WebSocket client...");
    unwrap!(spawner.spawn(pixelblaze_task(net_stack, rng,)));

    // Spawn LED control tasks on Core 1 to avoid blocking WiFi operations
    // This ensures smooth network communication while driving the LED matrix
    info!("💡 Starting LED control on Core 1...");
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor = CORE1_EXECUTOR.init(Executor::new());

            executor.run(|spawner| {
                // Configure I2C for NeoTrellis communication
                // Hardware: SDA=Pin6, SCL=Pin7, 100kHz frequency
                let mut config = i2c::Config::default();
                config.frequency = I2C_FREQUENCY;
                let i2c = i2c::I2c::new_blocking(p.I2C1, p.PIN_7, p.PIN_6, config);

                // TODO: Complete NeoTrellis integration
                // Currently blocked on async I2C driver compatibility
                // The neotrellis_task needs async I2C but we're using blocking I2C here
                // Options:
                // 1. Convert to async I2C (requires async executor on Core 1)
                // 2. Use blocking I2C in a loop with yields
                // 3. Implement custom async wrapper
                todo!("Integrate neotrellis_task with proper async I2C handling");
                // unwrap!(spawner.spawn(neotrellis_task(i2c)))
            });
        },
    );
}
