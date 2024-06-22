use adafruit_seesaw::{
    devices::{NeoTrellis, SeesawDevice, SeesawDeviceInit},
    prelude::NeopixelModule,
    SeesawError, SeesawRefCell,
};
use defmt::{info, Debug2Format};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};

pub(crate) const NEOTRELLIS_PIXELS: usize = 16;

#[derive(defmt::Format, Default)]
pub(crate) struct Rgb {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

pub(crate) enum Control {
    SyncFrame([Rgb; NEOTRELLIS_PIXELS]),
}

pub(crate) const MAX_CONTROL: usize = 5; // max 5 control messages waiting
pub(crate) static CONTROL_CHANNEL: Channel<CriticalSectionRawMutex, Control, MAX_CONTROL> =
    Channel::new();

pub(crate) const I2C_FREQUENCY: u32 = 100_000;

#[embassy_executor::task]
pub(crate) async fn neotrellis_task(
    i2c: embassy_rp::i2c::I2c<'static, embassy_rp::peripherals::I2C1, embassy_rp::i2c::Blocking>,
) -> ! {
    let delay = embassy_time::Delay;
    let seesaw = SeesawRefCell::new(delay, i2c);

    loop {
        if let Err(e) = drive_neotrellis(seesaw.acquire_driver()).await {
            info!(
                "neotrellis: task crashed: {}. restarting in 2 seconds",
                Debug2Format(&e)
            );
            Timer::after(Duration::from_secs(2)).await;
        }
    }
}

async fn drive_neotrellis<Seesaw: adafruit_seesaw::Driver>(
    seesaw: Seesaw,
) -> Result<(), SeesawError<Seesaw::Error>> {
    let mut neotrellis = NeoTrellis::new_with_default_addr(seesaw).init()?;
    let receiver = CONTROL_CHANNEL.receiver();

    loop {
        let Control::SyncFrame(preview_frame) = receiver.receive().await;

        for (n, Rgb { r, g, b }) in preview_frame.iter().enumerate() {
            neotrellis.set_nth_neopixel_color(
                n.try_into().expect("Failed to convert pixel index"),
                *r,
                *g,
                *b,
            )?;
        }
        neotrellis.sync_neopixel()?;
        // for evt in neotrellis.poll()? {
        //     info!("neutrellis: Event: x={} y={}", evt.x, evt.y,);
        //     match evt.event {
        //         EventType::Pressed => {
        //             neotrellis.set_nth_neopixel_color(
        //                 ((evt.y * 4) + evt.x).into(),
        //                 0xf,
        //                 0xf,
        //                 0xf,
        //             )?;
        //             neotrellis.sync_neopixel()?;
        //         }
        //         EventType::Released => {
        //             neotrellis.set_nth_neopixel_color(
        //                 ((evt.y * 4) + evt.x).into(),
        //                 evt.x,
        //                 0xf,
        //                 evt.y,
        //             )?;
        //             neotrellis.sync_neopixel()?;
        //         }
        //         _ => {}
        //     };
        // }
    }
}
