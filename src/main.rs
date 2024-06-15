#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

mod neotrellis;
mod pixelblaze;
mod wifi;

use defmt::{info, unwrap};
use embassy_executor::{Executor, Spawner};
use embassy_rp::multicore::spawn_core1;
use neotrellis::drive_neotrellis;
use pixelblaze::pixelblaze_websocket;
use static_cell::StaticCell;
use wifi::init_wifi;
use {defmt_rtt as _, panic_probe as _};

static mut CORE1_STACK: embassy_rp::multicore::Stack<4096> = embassy_rp::multicore::Stack::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let p = embassy_rp::init(Default::default());

    let net_stack = init_wifi(
        spawner, p.PIN_23, p.PIN_25, p.PIO0, p.PIN_24, p.PIN_29, p.DMA_CH0,
    )
    .await;

    // Pixelblaze websocket
    unwrap!(spawner.spawn(pixelblaze_websocket(net_stack)));

    // Non-Async code runs in a thread on the second CPU core
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor = CORE1_EXECUTOR.init(Executor::new());

            executor.run(|spawner| {
                let mut config: embassy_rp::i2c::Config = embassy_rp::i2c::Config::default();
                config.frequency = 50_000;
                let i2c = embassy_rp::i2c::I2c::new_blocking(p.I2C1, p.PIN_7, p.PIN_6, config);

                // TODO: Improve efficiancy by driving neotrellis asynchronously.
                unwrap!(spawner.spawn(drive_neotrellis(i2c)))
            });
        },
    );
}
