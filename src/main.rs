#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]
#![feature(impl_trait_in_assoc_type)]

mod animate;
mod neotrellis;
mod pixelblaze;
mod wifi;

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

static mut CORE1_STACK: embassy_rp::multicore::Stack<4096> = embassy_rp::multicore::Stack::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let mut rng = SmallRng::from_seed(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef_u128.to_le_bytes());

    let p = embassy_rp::init(Default::default());

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

    let net_stack = init_wifi(spawner, spi, pwr, rng.next_u64()).await;

    // Pixelblaze websocket
    unwrap!(spawner.spawn(pixelblaze_task(net_stack, rng,)));

    // Non-Async code runs in a thread on the second CPU core
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor = CORE1_EXECUTOR.init(Executor::new());

            executor.run(|spawner| {
                let mut config = i2c::Config::default();
                config.frequency = I2C_FREQUENCY;
                let i2c = i2c::I2c::new_blocking(p.I2C1, p.PIN_7, p.PIN_6, config);

                // TODO: Improve efficiancy by driving neotrellis asynchronously.
                unwrap!(spawner.spawn(neotrellis_task(i2c)))
            });
        },
    );
}
