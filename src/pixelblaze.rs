//! # Pixelblaze WebSocket Client
//!
//! Implements the Pixelblaze WebSocket protocol for real-time LED pattern preview.
//! Connects to the Pixelblaze controller (typically 192.168.4.1:81) and streams
//! RGB frame data to the NeoTrellis LED matrix.
//!
//! ## Protocol
//! - **Text**: JSON commands for config/control.
//! - **Binary**: Raw RGB frame data.
//!   Format: `[message_type: u8, r1: u8, g1: u8, b1: u8, ...]`
//!   (Preview Frame = Type 5)

use core::cell::Cell;
use core::cmp::min;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::str::{from_utf8, FromStr};
use edge_net::nal::TcpSplit;
use futures::try_join;

use defmt::{error, info, warn, Debug2Format};
use edge_http::io::client::Connection;
use edge_http::ws::{MAX_BASE64_KEY_LEN, MAX_BASE64_KEY_RESPONSE_LEN, NONCE_LEN};
use edge_nal_embassy::{TcpSocket, TcpSocketRead, TcpSocketWrite};
use edge_ws::{FrameHeader, FrameType};
use embassy_net::driver::Driver;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use heapless::String;
use rand::{rngs::SmallRng, RngCore};

use crate::neotrellis::{self, Rgb, NEOTRELLIS_PIXELS};

// Pixelblaze connection configuration
// Default Pixelblaze access point configuration when running in AP mode
const PIXELBLAZE_WS_HOST: &str = "192.168.4.1:81";        // WebSocket host:port
const PIXELBLAZE_WS_ORIGIN: &str = "http://192.168.4.1";  // HTTP origin for WebSocket upgrade
const PIXELBLAZE_WS_URI: &str = "/";                      // WebSocket URI path
const PIXELBLAZE_PORT: u16 = 81;                          // WebSocket port (default Pixelblaze)
const PIXELBLAZE_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 4, 1)); // Static IP

// Network resource constraints
const MAX_SOCKETS: usize = 1; // Only one TCP socket allowed (memory constraint)

const MAX_CONTROL: usize = 32; // Maximum control messages queued in channel

/// Control commands for the Pixelblaze WebSocket client.
pub(crate) enum Control {
    /// Send a WebSocket pong frame (response to ping)
    SendPong,
    /// Subscribe to real-time preview frames from Pixelblaze
    SubscribePreviewFrames,
    /// Request current Pixelblaze configuration
    GetConfig,
    /// Close the WebSocket connection gracefully
    Close,
    /// Set the active pattern on Pixelblaze (not fully implemented)
    SetActivePattern,
}

/// Channel for sending control commands to the Pixelblaze client.
pub(crate) static PIXELBLAZE_CONTROL_CHANNEL: Channel<
    CriticalSectionRawMutex,
    Control,
    MAX_CONTROL,
> = Channel::new();

/// Pixelblaze client error types
#[derive(Debug, defmt::Format)]
pub enum Error {
    /// General error (network, protocol, etc.)
    Error,
    /// Connection was closed (graceful or by remote)
    Close,
}

impl<E> From<edge_ws::Error<E>> for Error {
    fn from(_: edge_ws::Error<E>) -> Self {
        Error::Error
    }
}
impl<E> From<edge_http::io::Error<E>> for Error {
    fn from(_: edge_http::io::Error<E>) -> Self {
        Error::Error
    }
}

/// Pixelblaze binary message types.
#[derive(defmt::Format, PartialEq, Eq)]
pub enum PixelblazeMessageType {
    // Messages from Web UI to Pixelblaze
    /// Upload pattern source code to Pixelblaze
    PutSourceCode,
    /// Upload compiled bytecode to Pixelblaze
    PutByteCode,
    /// Send preview image data
    PreviewImage,
    /// Request list of available patterns
    GetProgramList,
    /// Upload pixel mapping configuration
    PutPixelMap,

    // Messages from Pixelblaze to Web UI
    /// **Most important**: Real-time RGB frame data for LED preview
    /// Format: [5, r1, g1, b1, r2, g2, b2, ...]
    PreviewFrame,
    /// Response with pattern source code
    GetSourceCode,

    // Bidirectional messages
    /// LED strip expander configuration
    ExpanderConfig,

    /// Unknown or unsupported message type
    Unknown(u8),
}

impl From<u8> for PixelblazeMessageType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::PutSourceCode,
            3 => Self::PutByteCode,
            4 => Self::PreviewImage,
            5 => Self::PreviewFrame,
            6 => Self::GetSourceCode,
            7 => Self::GetProgramList,
            8 => Self::PutPixelMap,
            9 => Self::ExpanderConfig,
            v => Self::Unknown(v),
        }
    }
}

/// Main Pixelblaze WebSocket client task.
///
/// Manages connection, frame streaming, and control commands.
/// Implements automatic reconnection with exponential backoff.
#[embassy_executor::task]
pub(crate) async fn pixelblaze_task(
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
    rng: SmallRng,
) -> ! {
    // Allocate network buffers for TCP communication
    let tcpbuf = edge_nal_embassy::TcpBuffers::new();
    let mut buf = [0_u8; 2048]; // Buffer for WebSocket frames
    let mut nonce = [0_u8; NONCE_LEN]; // WebSocket handshake nonce

    // Main connection loop - never exits
    loop {
        info!("pixelblaze: 🔌 Attempting connection to Pixelblaze...");
        let mut tcp = edge_nal_embassy::Tcp::<_, MAX_SOCKETS>::new(stack, &tcpbuf);

        match PixelStreamer::connect(&mut tcp, rng.clone(), &mut buf, &mut nonce).await {
            Ok((pixel_streamer, socket)) => {
                info!("pixelblaze: ✅ WebSocket connection established!");

                // Run the main communication loop
                if pixel_streamer
                    .communicate(socket, rng.clone())
                    .await
                    .is_err()
                {
                    warn!("pixelblaze: ❌ WebSocket communication failed");
                }
            }
            Err(_) => {
                warn!("pixelblaze: ❌ Failed to establish WebSocket connection");
            }
        }

        // Wait before reconnecting (exponential backoff could be added here)
        warn!("pixelblaze: 🔄 Reconnecting in 5 seconds...");
        Timer::after(Duration::from_secs(5)).await;
    }
}

/// Core WebSocket client for Pixelblaze communication.
///
/// Manages the streaming connection and performance metrics.
struct PixelStreamer {
    /// Currently active pattern ID (if known)
    current_pattern_id: Cell<Option<String<17>>>,
    /// Currently active pattern name (if known)
    current_pattern_name: Cell<Option<String<50>>>,
    /// Total frames received since connection (for FPS calculation)
    received_frames: Cell<u64>,
    /// Total frames dropped due to processing backlog
    dropped_frames: Cell<u64>,
}
impl<'b> PixelStreamer {
    /// Establish WebSocket connection to Pixelblaze.
    async fn connect<'d, D>(
        tcp: &'d mut edge_nal_embassy::Tcp<'d, D, MAX_SOCKETS>,
        mut rng: SmallRng,
        rx_buf: &'d mut [u8],
        nonce: &'d mut [u8; NONCE_LEN],
    ) -> Result<(PixelStreamer, TcpSocket<'d, MAX_SOCKETS, 1024, 1024>), Error>
    where
        D: Driver,
    {
        // Create HTTP connection to Pixelblaze
        let mut conn: Connection<_> =
            Connection::new(rx_buf, tcp, SocketAddr::new(PIXELBLAZE_IP, PIXELBLAZE_PORT));

        // Generate random nonce for WebSocket handshake security
        rng.fill_bytes(nonce);

        // Step 1: Send WebSocket upgrade request
        let mut buf = [0_u8; MAX_BASE64_KEY_LEN];
        conn.initiate_ws_upgrade_request(
            Some(PIXELBLAZE_WS_HOST),   // Host header
            Some(PIXELBLAZE_WS_ORIGIN), // Origin header
            PIXELBLAZE_WS_URI,          // Request URI (/)
            None,                       // No subprotocol
            nonce,                      // Security nonce
            &mut buf,
        )
        .await?;

        // Step 2: Read HTTP response headers
        conn.initiate_response().await?;

        // Step 3: Validate WebSocket acceptance
        let mut buf = [0_u8; MAX_BASE64_KEY_RESPONSE_LEN];
        if !conn.is_ws_upgrade_accepted(nonce, &mut buf)? {
            warn!("pixelblaze: ❌ WebSocket upgrade rejected by server");
            return Err(Error::Error);
        }

        // Step 4: Complete the handshake
        conn.complete().await?;

        // Extract raw TCP socket
        let (socket, _) = conn.release();

        Ok((
            PixelStreamer {
                current_pattern_name: Cell::new(None),
                current_pattern_id: Cell::new(None),
                received_frames: Cell::new(0),
                dropped_frames: Cell::new(0),
            },
            socket,
        ))
    }

    /// Main WebSocket communication loop.
    ///
    /// Runs monitoring, sending, and receiving tasks concurrently.
    async fn communicate(
        &self,
        mut socket: TcpSocket<'b, MAX_SOCKETS, 1024, 1024>,
        rng: SmallRng,
    ) -> Result<(), Error> {
        info!("pixelblaze: 🚀 WebSocket active, starting frame streaming");

        // Split socket for concurrent read/write operations
        let (rx, tx) = socket.split();

        // Run all three loops concurrently - any failure terminates all
        try_join!(
            self.monitoring_loop(),    // FPS monitoring and health checks
            self.send_loop(tx, rng),   // Outgoing command processing
            self.receive_loop(rx),     // Incoming frame processing
        )?;

        info!("pixelblaze: 🔌 Connection terminated, cleaning up");
        drop(socket);

        Ok(())
    }

    /// Connection monitoring and health management.
    async fn monitoring_loop(&self) -> Result<(), Error> {
        let control_commands = PIXELBLAZE_CONTROL_CHANNEL.sender();

        // Connection initialization sequence
        info!("pixelblaze: 🔧 Initializing connection...");
        Timer::after_millis(500).await;
        control_commands.send(Control::GetConfig).await;
        Timer::after_millis(500).await;

        // TODO: Conditional pattern activation
        // Only set active pattern if GetConfig shows no pattern is running
        // Current issue: SetActivePattern command doesn't seem to work reliably
        // Expected response: {"activeProgram":{"name":"Pattern Name","activeProgramId":"abc123","controls":{}},"sequencerMode":2,"runSequencer":true}
        // Actual response: {"activeProgram":{"name":"","activeProgramId":null,"controls":{}},"sequencerMode":2,"runSequencer":true}
        //control_commands.send(Control::SetActivePattern).await;

        // Frame rate monitoring variables
        let mut last_received_frames: u64 = 0;
        let mut last_dropped_frames: u64 = 0;

        // Main monitoring loop - runs every 10 seconds
        loop {
            Timer::after_millis(10000).await;

            // Calculate frame rates over the last 10-second window
            let new_received_frames = self.received_frames.get();
            let fps_received = new_received_frames - last_received_frames;
            last_received_frames = new_received_frames;

            let new_dropped_frames = self.dropped_frames.get();
            let fps_dropped = new_dropped_frames - last_dropped_frames;
            last_dropped_frames = new_dropped_frames;

            // Health check: if frame rate is too low, resubscribe
            if fps_received < 1 {
                // Use try_send to avoid blocking on channel full
                _ = control_commands.try_send(Control::SubscribePreviewFrames);
                warn!(
                    "pixelblaze: ⚠️  Low frame rate - rx={}/s dropped={}/s (resubscribing)",
                    fps_received / 10,
                    fps_dropped / 10,
                );
            } else {
                info!(
                    "pixelblaze: 📊 Frame rate healthy - rx={}/s dropped={}/s",
                    fps_received / 10,
                    fps_dropped / 10,
                );
            }
        }
    }

    /// WebSocket sending loop.
    async fn send_loop<'d>(
        &self,
        mut tx: TcpSocketWrite<'d>,
        mut rng: SmallRng,
    ) -> Result<(), Error> {
        let control_commands = PIXELBLAZE_CONTROL_CHANNEL.receiver();

        // Clear any stale commands from previous connections
        while control_commands.try_receive().is_ok() {}

        info!("pixelblaze: 📤 Send loop ready for commands");

        // Main command processing loop
        loop {
            match control_commands.receive().await {
                Control::SendPong => {
                    info!("pixelblaze: 🏓 Sending WebSocket pong");
                    let header = FrameHeader {
                        frame_type: FrameType::Pong,
                        payload_len: 0,
                        mask_key: rng.next_u32().into(), // Random mask for security
                    };
                    header.send(&mut tx).await?;
                }

                Control::SubscribePreviewFrames => {
                    info!("pixelblaze: 🎬 Subscribing to preview frames");
                    send_text_frame(&mut tx, &mut rng, r#"{"sendUpdates":true}"#).await?;
                }

                Control::GetConfig => {
                    info!("pixelblaze: ⚙️  Requesting configuration");
                    send_text_frame(&mut tx, &mut rng, r#"{"getConfig":true}"#).await?;
                }

                Control::SetActivePattern => {
                    info!("pixelblaze: 🎨 Setting active pattern");
                    // TODO: Make pattern ID configurable instead of hardcoded
                    send_text_frame(
                        &mut tx,
                        &mut rng,
                        r#"{"setActivePattern":"kuJfFyCSkCKNasyNE"}"#,
                    )
                    .await?;

                    // Update local state tracking
                    self.current_pattern_name
                        .replace(Some(String::from_str("Editor: blink fade").unwrap()));
                    self.current_pattern_id
                        .replace(Some(String::from_str("kuJfFyCSkCKNasyNE").unwrap()));

                    // Send additional configuration commands
                    Timer::after_millis(100).await;
                    send_text_frame(&mut tx, &mut rng, r#"{"setControls":{}}"#).await?;
                    Timer::after_millis(100).await;
                    send_text_frame(&mut tx, &mut rng, r#"{"pause":false}"#).await?;
                }

                Control::Close => {
                    info!("pixelblaze: 👋 Sending close frame");
                    // Send WebSocket close frame to server
                    let header: FrameHeader = FrameHeader {
                        frame_type: FrameType::Close,
                        payload_len: 0,
                        mask_key: rng.next_u32().into(),
                    };
                    header.send(&mut tx).await?;
                    return Err(Error::Close);
                }
            }
        }
    }



        loop {
            // Read WebSocket frame header and payload
            let header = FrameHeader::recv(&mut rx).await?;
            let payload = header.recv_payload(&mut rx, &mut buf).await?;

            // Pixelblaze doesn't typically send fragmented frames
            if !header.frame_type.is_final() {
                warn!(
                    "pixelblaze: ⚠️  Unexpected fragmented frame: type={} payload={}",
                    Debug2Format(&header.frame_type),
                    Debug2Format(&payload)
                );
            }

            match header.frame_type {
                FrameType::Text(_) => {
                    if let Ok(payload_str) = from_utf8(payload) {
                        // Filter out high-frequency FPS status messages to reduce log spam
                        // TODO: Parse and handle FPS status messages properly
                        if !payload_str.starts_with(r#"{"fps""#) {
                            info!("pixelblaze: 📄 Text message: {}", payload_str);
                        }
                    } else {
                        warn!(
                            "pixelblaze: ⚠️  Invalid UTF-8 in text frame: {}",
                            payload,
                        );
                    }
                }
                FrameType::Binary(_) => {
                    let t = PixelblazeMessageType::from(payload[0]);
                    if t == PixelblazeMessageType::PreviewFrame {
                        // This is the critical path - RGB frame data for LED display
                        match neotrellis::Control::try_from(payload) {
                            Ok(neotrellis::Control::SyncFrame(frame)) => {
                                self.handle_preview_frame(frame)
                            }
                            Err(e) => {
                                error!("pixelblaze: ❌ Failed to parse preview frame: {}", e)
                            }
                        }
                    } else {
                        info!(
                            "pixelblaze: 🔲 Binary message type={} payload={}",
                            t, payload,
                        );
                    }
                }
                FrameType::Ping => {
                    info!("pixelblaze: 🏓 Received ping, sending pong");
                    control_commands.send(Control::SendPong).await;
                }
                FrameType::Pong => {
                    info!("pixelblaze: 🏓 Received pong (connection alive)");
                }
                FrameType::Close => {
                    info!("pixelblaze: 👋 Server closing connection");
                    control_commands.send(Control::Close).await;
                }
                FrameType::Continue(is_final) => {
                    warn!(
                        "pixelblaze: ⚠️  Unexpected continue frame is_final={} payload={}",
                        is_final, payload
                    );
                }
            }
        }
    }

    /// Process a preview frame from Pixelblaze.
    fn handle_preview_frame(&self, frame: [Rgb; NEOTRELLIS_PIXELS]) {
        // Debug logging (commented out to avoid spam at 60+ FPS)
        // let received_frames = self.received_frames.get();
        // if received_frames % 200 == 0 {
        //     info!(
        //         "pixelblaze: 🎨 Preview frame #{}: {:?}",
        //         received_frames, frame
        //     );
        // }

        // Update frame statistics
        let received_frames = self.received_frames.get();
        self.received_frames.set(received_frames + 1);

        // Forward to NeoTrellis (non-blocking to avoid slowdown)
        // If the channel is full, the frame is dropped and counted
        if neotrellis::CONTROL_CHANNEL
            .try_send(neotrellis::Control::SyncFrame(frame))
            .is_err()
        {
            // Frame dropped - NeoTrellis can't keep up
            self.dropped_frames.set(self.dropped_frames.get() + 1)
        }
    }
}

/// Send a text message to Pixelblaze via WebSocket.
async fn send_text_frame<'d>(
    mut tx: &mut TcpSocketWrite<'d>,
    rng: &mut SmallRng,
    text_message: &str,
) -> Result<(), Error> {
    let header = FrameHeader {
        frame_type: FrameType::Text(false), // Not final frame of message
        payload_len: text_message.as_bytes().len() as _,
        mask_key: rng.next_u32().into(), // Random mask for security
    };

    info!("pixelblaze: 📤 Sending: {}", text_message);

    // Send frame header then payload
    header.send(&mut tx).await?;
    header
        .send_payload(&mut tx, text_message.as_bytes())
        .await?;

    Ok(())
}

/// Error types for preview frame parsing
#[derive(defmt::Format)]
pub(crate) enum PreviewFrameErr {
    /// Frame data is invalid or corrupted
    Invalid,
}

/// Convert Pixelblaze binary preview frame to NeoTrellis control message.
impl TryFrom<&[u8]> for neotrellis::Control {
    type Error = PreviewFrameErr;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        // Validate minimum frame size and message type
        if value.len() < 4
            || PixelblazeMessageType::from(value[0]) != PixelblazeMessageType::PreviewFrame
        {
            return Err(PreviewFrameErr::Invalid);
        }

        // RGB data starts after the message type byte
        let rgb_bytes = value.len() - 1;

        // Ensure we have complete RGB triplets (no partial pixels)
        if rgb_bytes % 3 != 0 {
            return Err(PreviewFrameErr::Invalid);
        }

        let mut preview_frame: [Rgb; NEOTRELLIS_PIXELS] = Default::default();

        // Convert RGB bytes to pixel array
        // TODO: Use Iterator.collect() once no_std supports it better
        let pixels = rgb_bytes / 3;
        #[allow(clippy::needless_range_loop)]
        for i in 0..min(NEOTRELLIS_PIXELS, pixels) {
            let position = 1 + (i * 3); // Skip message type byte
            let [r, g, b] = value[position..position + 3] else {
                return Err(PreviewFrameErr::Invalid);
            };
            preview_frame[i] = Rgb { r, g, b }
        }

        Ok(neotrellis::Control::SyncFrame(preview_frame))
    }
}
