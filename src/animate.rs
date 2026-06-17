//! # Fallback Animations
//!
//! Visual feedback animations for the NeoTrellis LED matrix during system operations.

use defmt::info;
use embassy_time::{Duration, Timer};

use crate::neotrellis::{self, Rgb, NEOTRELLIS_PIXELS};

/// ASCII-art 4x4 LED pattern. 'w'/'x' = white, 'r' = red, 'g' = green, 'b' = blue, others = off.
pub(crate) struct RGBPattern {
    /// First row (top) of the 4x4 matrix
    pub(crate) r1: &'static str,
    /// Second row of the 4x4 matrix
    pub(crate) r2: &'static str,
    /// Third row of the 4x4 matrix
    pub(crate) r3: &'static str,
    /// Fourth row (bottom) of the 4x4 matrix
    pub(crate) r4: &'static str,
}

/// Low brightness (8%) to preserve battery.
const BRIGHTNESS: u8 = 20;

/// Convert ASCII pattern to linear RGB array.
impl From<RGBPattern> for [Rgb; NEOTRELLIS_PIXELS] {
    fn from(val: RGBPattern) -> Self {
        // Character-to-RGB color mapping function
        let to_rgb = |p: char| {
            match p {
                'w' | 'x' => Rgb {
                    r: BRIGHTNESS,
                    g: BRIGHTNESS,
                    b: BRIGHTNESS,
                }, // White/bright
                'r' => Rgb {
                    r: BRIGHTNESS,
                    g: 0,
                    b: 0,
                }, // Red
                'g' => Rgb {
                    r: 0,
                    g: BRIGHTNESS,
                    b: 0,
                }, // Green
                'b' => Rgb {
                    r: 0,
                    g: 0,
                    b: BRIGHTNESS,
                }, // Blue
                _ => Rgb { r: 0, g: 0, b: 0 }, // Off/black for any other character
            }
        };

        let mut frame = <[Rgb; NEOTRELLIS_PIXELS]>::default();

        // Convert 4x4 ASCII pattern to linear RGB array
        // Chain all rows together and convert each character to RGB
        for (i, pixel) in val
            .r1
            .chars()
            .take(4) // Ensure exactly 4 characters per row
            .map(to_rgb)
            .chain(val.r2.chars().take(4).map(to_rgb))
            .chain(val.r3.chars().take(4).map(to_rgb))
            .chain(val.r4.chars().take(4).map(to_rgb))
            .enumerate()
        {
            if i >= NEOTRELLIS_PIXELS {
                break; // Safety check - shouldn't happen with proper input
            }
            frame[i] = pixel
        }
        frame
    }
}

/// Animation frame delay.
const SLEEP: u64 = 600;

/// Rotating diagonal line animation for connection feedback.
pub(crate) async fn wait_animation() -> ! {
    let sender = neotrellis::CONTROL_CHANNEL.sender();

    info!("animate: 🌀 Starting WiFi connection animation");

    loop {
        // Frame 1: Main diagonal (top-left to bottom-right)
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "x...", // LED at (0,0)
                r2: ".x..", // LED at (1,1)
                r3: "..x.", // LED at (2,2)
                r4: "...x", // LED at (3,3)
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;

        // Frame 2: Shifted diagonal pattern
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: ".x..", // LED at (0,1)
                r2: "x...", // LED at (1,0)
                r3: "...x", // LED at (2,3)
                r4: "..x.", // LED at (3,2)
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;

        // Frame 3: Anti-diagonal (top-right to bottom-left)
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "..x.", // LED at (0,2)
                r2: "...x", // LED at (1,3)
                r3: "x...", // LED at (2,0)
                r4: ".x..", // LED at (3,1)
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;

        // Frame 4: Shifted anti-diagonal pattern
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "...x", // LED at (0,3)
                r2: "..x.", // LED at (1,2)
                r3: ".x..", // LED at (2,1)
                r4: "x...", // LED at (3,0)
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;

        // Frame 5: Return to shifted diagonal (creates smooth loop)
        _ = sender.try_send(neotrellis::Control::SyncFrame(
            RGBPattern {
                r1: "..x.", // LED at (0,2)
                r2: "...x", // LED at (1,3)
                r3: "x...", // LED at (2,0)
                r4: ".x..", // LED at (3,1)
            }
            .into(),
        ));
        Timer::after(Duration::from_millis(SLEEP)).await;
    }
}
