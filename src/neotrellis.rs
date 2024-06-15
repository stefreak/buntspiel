use adafruit_seesaw::{
    devices::{NeoTrellis, SeesawDevice, SeesawDeviceInit},
    prelude::{EventType, KeypadModule, NeopixelModule},
    SeesawRefCell,
};
use defmt::info;
use embassy_time::{Duration, Timer};

#[embassy_executor::task]
pub(crate) async fn drive_neotrellis(
    i2c: embassy_rp::i2c::I2c<'static, embassy_rp::peripherals::I2C1, embassy_rp::i2c::Blocking>,
) {
    let delay = embassy_time::Delay;
    let seesaw = SeesawRefCell::new(delay, i2c);
    let mut neotrellis = NeoTrellis::new_with_default_addr(seesaw.acquire_driver())
        .init()
        .expect("Failed to start neotrellis");

    loop {
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
