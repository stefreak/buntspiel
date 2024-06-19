use core::cell::Cell;
use core::cmp::min;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::str::from_utf8;
use core::{u8, usize};
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
use heapless::Vec;
use rand::{rngs::SmallRng, RngCore};

const BUNTSPIEL_PIXELS: usize = 16;

const PIXELBLAZE_WS_HOST: &str = "192.168.4.1:81";
const PIXELBLAZE_WS_ORIGIN: &str = "http://192.168.4.1";
const PIXELBLAZE_WS_URI: &str = "/";
const PIXELBLAZE_PORT: u16 = 81;
const PIXELBLAZE_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 4, 1));

// allow max one open TCP socket
const MAX_SOCKETS: usize = 1;

pub(crate) enum PixelData {
    PreviewFrame(PreviewFrame),
}
pub(crate) const MAX_FRAMES: usize = 1; // max one frame waiting in channel
pub(crate) static PIXELBLAZE_FRAME_CHANNEL: Channel<
    CriticalSectionRawMutex,
    PixelData,
    MAX_FRAMES,
> = Channel::new();

const MAX_CONTROL: usize = 32; // max control messages waiting in channel
pub(crate) enum PixelblazeControl {
    SendPong,
    SubscribePreviewFrames,
    GetConfig,
    Close,
}
pub(crate) static PIXELBLAZE_CONTROL_CHANNEL: Channel<
    CriticalSectionRawMutex,
    PixelblazeControl,
    MAX_CONTROL,
> = Channel::new();

#[derive(Debug, defmt::Format)]
pub enum Error {
    Error,
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

#[derive(defmt::Format, PartialEq, Eq)]
pub enum PixelblazeMessageType {
    // from webUI to Pixeblaze
    PutSourceCode,
    PutByteCode,
    PreviewImage,
    GetProgramList,
    PutPixelMap,

    // from pixelblaze to webui
    PreviewFrame,
    GetSourceCode,

    // both directions
    ExpanderConfig,

    // unknown message type
    Unknown(u8),
}

impl From<u8> for PixelblazeMessageType {
    fn from(value: u8) -> Self {
        match value {
            01 => Self::PutSourceCode,
            03 => Self::PutByteCode,
            04 => Self::PreviewImage,
            05 => Self::PreviewFrame,
            06 => Self::GetSourceCode,
            07 => Self::GetProgramList,
            08 => Self::PutPixelMap,
            09 => Self::ExpanderConfig,
            v => Self::Unknown(v),
        }
    }
}

#[embassy_executor::task]
pub(crate) async fn pixelblaze_task(
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
    rng: SmallRng,
) -> ! {
    let tcpbuf = edge_nal_embassy::TcpBuffers::new();
    let mut buf = [0_u8; 2048];
    let mut nonce = [0_u8; NONCE_LEN];

    loop {
        info!("pixelblaze: connecting...");
        let mut tcp = edge_nal_embassy::Tcp::<_, MAX_SOCKETS>::new(&stack, &tcpbuf);
        match PixelStreamer::connect(&mut tcp, rng.clone(), &mut buf, &mut nonce).await {
            Ok((pixel_streamer, socket)) => {
                if let Err(_) = pixel_streamer.communicate(socket, rng.clone()).await {
                    warn!("pixelblaze: WS communication failed",);
                }
            }
            Err(_) => {
                warn!("pixelblaze: failed establishing ws conn. Trying again in 5 seconds",);
            }
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}

struct PixelStreamer {
    received_frames: Cell<u64>,
    dropped_frames: Cell<u64>,
}
impl<'b> PixelStreamer {
    async fn connect<'d, D>(
        tcp: &'d mut edge_nal_embassy::Tcp<'d, D, MAX_SOCKETS>,
        mut rng: SmallRng,
        rx_buf: &'d mut [u8],
        nonce: &'d mut [u8; NONCE_LEN],
    ) -> Result<(PixelStreamer, TcpSocket<'d, MAX_SOCKETS, 1024, 1024>), Error>
    where
        D: Driver,
    {
        let mut conn: Connection<_> =
            Connection::new(rx_buf, tcp, SocketAddr::new(PIXELBLAZE_IP, PIXELBLAZE_PORT));

        rng.fill_bytes(nonce);

        let mut buf = [0_u8; MAX_BASE64_KEY_LEN];
        conn.initiate_ws_upgrade_request(
            Some(PIXELBLAZE_WS_HOST),
            Some(PIXELBLAZE_WS_ORIGIN),
            PIXELBLAZE_WS_URI,
            None,
            nonce,
            &mut buf,
        )
        .await?;
        conn.initiate_response().await?;
        let mut buf = [0_u8; MAX_BASE64_KEY_RESPONSE_LEN];
        if !conn.is_ws_upgrade_accepted(&nonce, &mut buf)? {
            warn!("pixelblaze: WS upgrade not accepted");
            return Err(Error::Error);
        }

        conn.complete().await?;

        // Now we have the TCP socket in a state where it can be operated as a WS connection
        // Send some traffic to a WS echo server and read it back

        let (socket, _) = conn.release();

        return Ok((
            PixelStreamer {
                received_frames: Cell::new(0),
                dropped_frames: Cell::new(0),
            },
            socket,
        ));
    }

    async fn communicate(
        self: &Self,
        mut socket: TcpSocket<'b, MAX_SOCKETS, 1024, 1024>,
        rng: SmallRng,
    ) -> Result<(), Error> {
        info!("pixelblaze: Connection upgraded to WS, starting traffic now");

        let (rx, tx) = socket.split();

        try_join!(
            self.monitoring_loop(),
            self.send_loop(tx, rng),
            self.receive_loop(rx),
        )?;

        info!("pixelblaze: Closing connection");
        drop(socket);

        return Ok(());
    }

    async fn monitoring_loop(self: &Self) -> Result<(), Error> {
        let control_commands = PIXELBLAZE_CONTROL_CHANNEL.sender();

        // Ask for config first thing in the morning
        control_commands.send(PixelblazeControl::GetConfig).await;

        let mut last_received_frames: u64 = 0;
        let mut last_dropped_frames: u64 = 0;

        loop {
            Timer::after_millis(1000).await;

            // calculate received fps
            let new_received_frames = self.received_frames.get();
            let fps_received = new_received_frames - last_received_frames;
            last_received_frames = new_received_frames;
            let new_dropped_frames = self.dropped_frames.get();
            let fps_dropped = new_dropped_frames - last_dropped_frames;
            last_dropped_frames = new_dropped_frames;

            info!(
                "pixelblaze: received {} frames (dropped {} frames)",
                fps_received, fps_dropped,
            );

            if fps_received < 1 {
                // use try_send to keep FPS timing accuracy
                _ = control_commands.try_send(PixelblazeControl::SubscribePreviewFrames)
            }
        }
    }

    async fn send_loop<'d>(
        self: &Self,
        mut tx: TcpSocketWrite<'d>,
        mut rng: SmallRng,
    ) -> Result<(), Error> {
        let control_commands = PIXELBLAZE_CONTROL_CHANNEL.receiver();
        loop {
            match control_commands.receive().await {
                PixelblazeControl::SendPong => {
                    info!("pixelblaze: Sending pong");
                    let header = FrameHeader {
                        frame_type: FrameType::Pong,
                        payload_len: 0,
                        mask_key: rng.next_u32().into(),
                    };
                    header.send(&mut tx).await?;
                }
                PixelblazeControl::SubscribePreviewFrames => {
                    send_text_frame(&mut tx, &mut rng, r#"{"sendUpdates":true}"#).await?;
                }
                PixelblazeControl::GetConfig => {
                    send_text_frame(&mut tx, &mut rng, r#"{"getConfig":true}"#).await?;
                }
                PixelblazeControl::Close => {
                    // Inform the server we are closing the connection
                    let header: FrameHeader = FrameHeader {
                        frame_type: FrameType::Close,
                        payload_len: 0,
                        mask_key: rng.next_u32().into(),
                    };

                    header.send(&mut tx).await?;

                    info!("pixelblaze: Closing");
                    return Err(Error::Close);
                }
            }
        }
    }

    async fn receive_loop<'d>(self: &Self, mut rx: TcpSocketRead<'d>) -> Result<(), Error> {
        let mut buf = [0_u8; 2048];

        let control_commands = PIXELBLAZE_CONTROL_CHANNEL.sender();

        loop {
            let header = FrameHeader::recv(&mut rx).await?;
            let payload = header.recv_payload(&mut rx, &mut buf).await?;

            if !header.frame_type.is_final() {
                warn!(
                    "pixelblaze: Got unexpected fragmented frame: type={} payload={}",
                    Debug2Format(&header.frame_type),
                    Debug2Format(&payload)
                );
            }

            match header.frame_type {
                FrameType::Text(_) => {
                    if let Ok(payload_str) = from_utf8(payload) {
                        info!("pixelblaze: Got text frame payload={}", payload_str,);
                    } else {
                        info!(
                            "pixelblaze: Got text frame (UTF8 decoding failed) payload={}",
                            payload,
                        );
                    }
                }
                FrameType::Binary(_) => {
                    let t = PixelblazeMessageType::from(payload[0]);
                    if t == PixelblazeMessageType::PreviewFrame {
                        match PreviewFrame::try_from(payload) {
                            Ok(preview_frame) => self.handle_preview_frame(preview_frame),
                            Err(e) => {
                                error!("pixelblaze: Could not parse preview frame: {}", e)
                            }
                        }
                    } else {
                        info!(
                            "pixelblaze: Got binary frame type={} payload={}",
                            t, payload,
                        );
                    }
                }
                FrameType::Ping => {
                    info!("pixelblaze: Got ping frame",);
                    control_commands.send(PixelblazeControl::SendPong).await;
                }
                FrameType::Pong => {
                    info!("pixelblaze: Got pong frame",);
                }
                FrameType::Close => {
                    info!("pixelblaze: Got close frame",);
                    control_commands.send(PixelblazeControl::Close).await;
                }
                FrameType::Continue(is_final) => {
                    warn!(
                        "pixelblaze: Got unexpected continue frame is_final={} payload={}",
                        is_final, payload
                    );
                }
            }
        }
    }

    fn handle_preview_frame(self: &Self, preview_frame: PreviewFrame) -> () {
        let preview_frames = PIXELBLAZE_FRAME_CHANNEL.sender();

        let received_frames = self.received_frames.get();
        if received_frames % 200 == 0 {
            info!(
                "pixelblaze: Received PreviewFrame (sample {}): {}",
                self.received_frames, preview_frame,
            );
        }

        if let Err(_) = preview_frames.try_send(PixelData::PreviewFrame(preview_frame)) {
            self.dropped_frames.set(self.dropped_frames.get() + 1)
        }

        self.received_frames.set(received_frames + 1);
    }
}

async fn send_text_frame<'d>(
    mut tx: &mut TcpSocketWrite<'d>,
    rng: &mut SmallRng,
    text_message: &str,
) -> Result<(), Error> {
    let header = FrameHeader {
        frame_type: FrameType::Text(false),
        payload_len: text_message.as_bytes().len() as _,
        mask_key: rng.next_u32().into(),
    };

    info!("Sending text frame with payload \"{}\"", text_message);
    header.send(&mut tx).await?;
    header
        .send_payload(&mut tx, text_message.as_bytes())
        .await?;

    return Ok(());
}

#[derive(defmt::Format)]
pub(crate) struct RGB {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

#[derive(defmt::Format)]
pub(crate) enum PreviewFrameErr {
    Invalid,
    Overflow,
}

#[derive(defmt::Format)]
pub(crate) struct PreviewFrame {
    pub(crate) relevant_pixels: Vec<RGB, BUNTSPIEL_PIXELS>,
}

impl TryFrom<&[u8]> for PreviewFrame {
    type Error = PreviewFrameErr;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 4
            || PixelblazeMessageType::from(value[0]) != PixelblazeMessageType::PreviewFrame
        {
            return Err(PreviewFrameErr::Invalid);
        }

        // check if number of RGB values are divisible by 3, otherwise frame is incomplete or invalid
        let rgb_bytes = value.len() - 1;
        if rgb_bytes % 3 != 0 {
            return Err(PreviewFrameErr::Invalid);
        }

        let mut preview_frame = PreviewFrame {
            relevant_pixels: Vec::new(),
        };
        let pixels = rgb_bytes / 3;
        for p in 0..min(BUNTSPIEL_PIXELS, pixels) {
            let position = p * 3;
            let [r, g, b] = value[position..position + 3] else {
                return Err(PreviewFrameErr::Invalid);
            };
            if let Err(_) = preview_frame.relevant_pixels.push(RGB { r, g, b }) {
                return Err(PreviewFrameErr::Overflow);
            }
        }

        return Ok(preview_frame);
    }
}

impl PreviewFrame {}
