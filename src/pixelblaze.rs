use core::cmp::min;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::str::from_utf8;
use core::u8;

use defmt::{error, info, warn};
use edge_http::io::client::Connection;
use edge_http::ws::{MAX_BASE64_KEY_LEN, MAX_BASE64_KEY_RESPONSE_LEN, NONCE_LEN};
use edge_nal_embassy::TcpSocket;
use edge_ws::{FrameHeader, FrameType};
use embassy_net::driver::Driver;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
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

const PIXELBLAZE_MSG_GET_CONFIG: &str = r#"{"getConfig":true}"#;
const PIXELBLAZE_MSG_SEND_UPDATES: &str = r#"{"sendUpdates":true}"#;

const N_SOCKETS: usize = 1;

pub(crate) enum PixelData {
    PreviewFrame(PreviewFrame),
}
pub(crate) static CHANNEL: Channel<ThreadModeRawMutex, PixelData, 64> = Channel::new();

#[derive(Debug, defmt::Format)]
pub enum Error {
    Error,
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
    mut rng: SmallRng,
) -> ! {
    let tcpbuf = edge_nal_embassy::TcpBuffers::new();
    let mut buf = [0_u8; 2048];
    let mut nonce = [0_u8; NONCE_LEN];

    loop {
        info!("pixelblaze: connecting...");
        let mut tcp = edge_nal_embassy::Tcp::<_, N_SOCKETS>::new(&stack, &tcpbuf);
        match PixelStreamer::connect(&mut tcp, &mut rng, &mut buf, &mut nonce).await {
            Ok(mut pixel_streamer) => {
                if let Err(_) = pixel_streamer.communicate_ws().await {
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

struct PixelStreamer<'b> {
    socket: TcpSocket<'b, N_SOCKETS, 1024, 1024>,
    buf: &'b mut [u8],
    rng: &'b mut SmallRng,
}

impl<'b> PixelStreamer<'b> {
    async fn connect<'d, D>(
        tcp: &'d mut edge_nal_embassy::Tcp<'d, D, N_SOCKETS>,
        rng: &'d mut SmallRng,
        buf: &'d mut [u8],
        nonce: &'d mut [u8; NONCE_LEN],
    ) -> Result<PixelStreamer<'d>, Error>
    where
        D: Driver,
    {
        let mut conn: Connection<_> =
            Connection::new(buf, tcp, SocketAddr::new(PIXELBLAZE_IP, PIXELBLAZE_PORT));

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

        let (socket, buf) = conn.release();

        return Ok(PixelStreamer { socket, buf, rng });
    }

    async fn communicate_ws(self: &mut Self) -> Result<(), Error> {
        info!("pixelblaze: Connection upgraded to WS, starting traffic now");

        let channel = CHANNEL.sender();

        // Sending data

        self.send_text_frame(PIXELBLAZE_MSG_GET_CONFIG).await?;
        self.send_text_frame(PIXELBLAZE_MSG_SEND_UPDATES).await?;

        let mut received_preview_frames: u32 = 0;

        // receive loop
        loop {
            let header = FrameHeader::recv(&mut self.socket).await?;
            let payload = header.recv_payload(&mut self.socket, self.buf).await?;

            match header.frame_type {
                FrameType::Text(fragmented) => {
                    if let Ok(payload_str) = from_utf8(payload) {
                        info!(
                            "pixelblaze: Got text frame fragmented={}, payload={}",
                            fragmented, payload_str,
                        );
                    } else {
                        info!(
                            "pixelblaze: Got text frame (UTF8 decoding failed) fragmented={}, payload={}",
                            fragmented, payload,
                        );
                    }
                }
                FrameType::Binary(fragmented) => {
                    let t = PixelblazeMessageType::from(payload[0]);
                    if t == PixelblazeMessageType::PreviewFrame {
                        match PreviewFrame::try_from(payload) {
                            Ok(preview_frame) => {
                                if received_preview_frames % 200 == 0 {
                                    info!(
                                        "pixelblaze: Got PreviewFrame (sample {}) fragmented={}, payload={}",
                                        received_preview_frames, fragmented, payload,
                                    );
                                }
                                if let Err(_) =
                                    channel.try_send(PixelData::PreviewFrame(preview_frame))
                                {
                                    error!("pixelblaze: failed sending preview frame: the channel is full.",)
                                }
                            }
                            Err(e) => {
                                error!("pixelblaze: Could not parse preview frame: {}", e)
                            }
                        }
                        (received_preview_frames, _) = received_preview_frames.overflowing_add(1);
                    } else {
                        info!(
                            "pixelblaze: Got binary frame fragmented={}, type={} payload={}",
                            fragmented, t, payload,
                        );
                    }
                }
                FrameType::Ping => {
                    info!("pixelblaze: Got ping frame",);
                    let header = FrameHeader {
                        frame_type: FrameType::Pong,
                        payload_len: 0,
                        mask_key: self.rng.next_u32().into(),
                    };

                    info!("pixelblaze: Sending pong");
                    header.send(&mut self.socket).await?;
                }
                FrameType::Pong => {
                    info!("pixelblaze: Got pong frame",);
                }
                FrameType::Close => {
                    info!("pixelblaze: Got close frame",);
                    // Inform the server we are closing the connection
                    let header: FrameHeader = FrameHeader {
                        frame_type: FrameType::Close,
                        payload_len: 0,
                        mask_key: self.rng.next_u32().into(),
                    };

                    info!("pixelblaze: Closing");

                    header.send(&mut self.socket).await?;

                    return Ok(());
                }
                FrameType::Continue(is_final) => {
                    info!(
                        "pixelblaze: Got continue frame is_final={} payload={}",
                        is_final, payload
                    );
                }
            }

            if !header.frame_type.is_final() {
                warn!("pixelblaze: Got unexpected fragmented frame");
            }
        }
    }

    async fn send_text_frame<'d>(self: &mut Self, text_message: &str) -> Result<(), Error> {
        let header = FrameHeader {
            frame_type: FrameType::Text(false),
            payload_len: text_message.as_bytes().len() as _,
            mask_key: self.rng.next_u32().into(),
        };

        info!("Sending text frame with payload \"{}\"", text_message);
        header.send(&mut self.socket).await?;
        header
            .send_payload(&mut self.socket, text_message.as_bytes())
            .await?;

        return Ok(());
    }
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
