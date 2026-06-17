//! # NeoTrellis 4x4 LED Matrix Driver
//!
//! Controls an Adafruit NeoTrellis 4x4 RGB LED matrix via I2C to display
//! real-time pattern previews from Pixelblaze.
//!
//! ## Hardware
//! - **Device**: NeoTrellis 4x4 (16 RGB LEDs + 16 buttons)
//! - **Interface**: I2C (SDA=Pin6, SCL=Pin7) on Pico W
//!
//! ## Communication
//! Receives RGB frames via `CONTROL_CHANNEL` from the Pixelblaze WebSocket client.

use adafruit_seesaw::{
    devices::{NeoTrellis, SeesawDevice, SeesawDeviceInit},
    prelude::NeopixelModule,
    SeesawError, SeesawRefCell,
};
use defmt::{info, Debug2Format};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};

/// Number of RGB LEDs in the NeoTrellis 4x4 matrix
pub(crate) const NEOTRELLIS_PIXELS: usize = 16;

/// RGB color value for a single LED pixel (0-255).
#[derive(defmt::Format, Default)]
pub(crate) struct Rgb {
    /// Red component (0-255)
    pub(crate) r: u8,
    /// Green component (0-255)
    pub(crate) g: u8,
    /// Blue component (0-255)
    pub(crate) b: u8,
}

/// Control messages for the NeoTrellis LED matrix.
pub(crate) enum Control {
    /// Complete frame of RGB data for the entire 4x4 matrix (row-major order).
    SyncFrame([Rgb; NEOTRELLIS_PIXELS]),
}

/// Maximum number of control messages that can be queued.
pub(crate) const MAX_CONTROL: usize = 5;

/// Channel for receiving LED control messages.
pub(crate) static CONTROL_CHANNEL: Channel<CriticalSectionRawMutex, Control, MAX_CONTROL> =
    Channel::new();

/// I2C communication frequency.
pub(crate) const I2C_FREQUENCY: u32 = 100_000;

/// Main NeoTrellis control task.
///
/// Runs on Core 1, processing LED frame updates from the control channel.
/// Restarts after a 2-second delay if I2C communication fails.
#[embassy_executor::task]
pub(crate) async fn neotrellis_task(
    i2c: embassy_rp::i2c::I2c<'static, embassy_rp::peripherals::I2C1, embassy_rp::i2c::Async>,
) -> ! {
    // Create delay provider for Seesaw initialization timing
    let delay = embassy_time::Delay;
    // Wrap I2C driver in Seesaw-compatible interface
    let seesaw = SeesawRefCell::new(delay, i2c);

    info!("neotrellis: 🚀 Starting NeoTrellis control loop");

    // Main task loop with error recovery
    loop {
        if let Err(e) = drive_neotrellis(seesaw.acquire_driver()).await {
            info!(
                "neotrellis: ❌ Driver error: {}. Restarting in 2 seconds...",
                Debug2Format(&e)
            );
            Timer::after(Duration::from_secs(2)).await;
        }
    }
}

/// Core NeoTrellis driver loop.
///
/// Initializes the device and processes frame updates from the control channel.
async fn drive_neotrellis<Seesaw: adafruit_seesaw::Driver>(
    seesaw: Seesaw,
) -> Result<(), SeesawError<Seesaw::Error>> {
    // Initialize NeoTrellis (default addr 0x2E)
    let mut neotrellis = NeoTrellis::new_with_default_addr(seesaw).init()?;
    info!("neotrellis: ✅ Hardware initialized successfully");

    // Get receiver for LED control messages
    let receiver = CONTROL_CHANNEL.receiver();

    info!("neotrellis: 🎨 Ready to display patterns");

    // Main frame processing loop
    loop {
        // Wait for new frame data from Pixelblaze
        let Control::SyncFrame(preview_frame) = receiver.receive().await;

        // Update all 16 LEDs (row-major order)
        for (n, Rgb { r, g, b }) in preview_frame.iter().enumerate() {
            neotrellis.set_nth_neopixel_color(
                n.try_into().expect("Pixel index out of range"), // Should never fail for 0-15
                *r,
                *g,
                *b,
            )?;
        }

        // Commit LED changes
        neotrellis.sync_neopixel()?;
        // TODO: Implement button event handling for VJ interface
        // This code would enable pattern selection via physical button presses
        //
        // for evt in neotrellis.poll()? {
        //     info!("neotrellis: 🔘 Button event at x={} y={}", evt.x, evt.y);
        //     match evt.event {
        //         EventType::Pressed => {
        //             // Button pressed - could trigger pattern selection
        //             let led_index = (evt.y * 4) + evt.x;
        //             neotrellis.set_nth_neopixel_color(
        //                 led_index.into(),
        //                 0xFF, 0xFF, 0xFF, // White feedback
        //             )?;
        //             neotrellis.sync_neopixel()?;
        //             // TODO: Send pattern selection command to Pixelblaze
        //         }
        //         EventType::Released => {
        //             // Button released - restore pattern color
        //             let led_index = (evt.y * 4) + evt.x;
        //             neotrellis.set_nth_neopixel_color(
        //                 led_index.into(),
        //                 evt.x * 0x40,  // Position-based color
        //                 0xFF,
        //                 evt.y * 0x40,
        //             )?;
        //             neotrellis.sync_neopixel()?;
        //         }
        //         _ => {
        //             // Other event types (hold, etc.)
        //         }
        //     };
        // }
    }
}
