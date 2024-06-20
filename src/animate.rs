use defmt::info;
use embassy_time::{Duration, Timer};

use crate::neotrellis::{self, NEOTRELLIS_PIXELS, RGB};

pub(crate) struct RGBPattern {
    pub(crate) r1: &'static str,
    pub(crate) r2: &'static str,
    pub(crate) r3: &'static str,
    pub(crate) r4: &'static str,
}

const BRIGHTNESS: u8 = 20;

impl Into<[RGB; NEOTRELLIS_PIXELS]> for RGBPattern {
    fn into(self) -> [RGB; NEOTRELLIS_PIXELS] {
        let to_rgb = |p: char| {
            if p == 'w' || p == 'x' {
                return RGB {
                    r: BRIGHTNESS,
                    g: BRIGHTNESS,
                    b: BRIGHTNESS,
                };
            }
            if p == 'r' {
                return RGB {
                    r: BRIGHTNESS,
                    g: 0,
                    b: 0,
                };
            }
            if p == 'g' {
                return RGB {
                    r: 0,
                    g: BRIGHTNESS,
                    b: 0,
                };
            }
            if p == 'b' {
                return RGB {
                    r: 0,
                    g: 0,
                    b: BRIGHTNESS,
                };
            }
            return RGB { r: 0, g: 0, b: 0 };
        };

        let mut frame = <[RGB; NEOTRELLIS_PIXELS]>::default();

        for (i, pixel) in self
            .r1
            .chars()
            .take(4)
            .map(to_rgb)
            .chain(self.r2.chars().take(4).map(to_rgb))
            .chain(self.r3.chars().take(4).map(to_rgb))
            .chain(self.r4.chars().take(4).map(to_rgb))
            .into_iter()
            .enumerate()
        {
            if i >= NEOTRELLIS_PIXELS {
                break;
            }
            frame[i] = pixel
        }
        return frame;
    }
}

const SLEEP: u64 = 600;

pub(crate) async fn wait_animation() -> ! {
    let sender = neotrellis::CONTROL_CHANNEL.sender();

    loop {
        info!("WAIT_ANIMATION////////WAIT_ANIMATION////////");
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "x...",
                r2: ".x..",
                r3: "..x.",
                r4: "...x",
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: ".x..",
                r2: "x...",
                r3: "...x",
                r4: "..x.",
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "..x.",
                r2: "...x",
                r3: "x...",
                r4: ".x..",
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "...x",
                r2: "..x.",
                r3: ".x..",
                r4: "x...",
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "..x.",
                r2: "...x",
                r3: "x...",
                r4: ".x..",
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;
    }
}
