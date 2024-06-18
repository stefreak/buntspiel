use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::str::from_utf8;
use core::u8;

use defmt::{info, warn};
use edge_http::io::client::Connection;
use edge_http::ws::{MAX_BASE64_KEY_LEN, MAX_BASE64_KEY_RESPONSE_LEN, NONCE_LEN};
use edge_nal_embassy::TcpSocket;
use edge_ws::{FrameHeader, FrameType};
use embassy_net::driver::Driver;
use embassy_time::{Duration, Timer};
use rand::{rngs::SmallRng, RngCore};

const PIXELBLAZE_WS_HOST: &str = "192.168.4.1:81";
const PIXELBLAZE_WS_ORIGIN: &str = "http://192.168.4.1";
const PIXELBLAZE_WS_URI: &str = "/";
const PIXELBLAZE_PORT: u16 = 81;
const PIXELBLAZE_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 4, 1));

const PIXELBLAZE_MSG_GET_CONFIG: &str = r#"{"getConfig":true}"#;

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

#[derive(defmt::Format)]
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
        let mut tcp = edge_nal_embassy::Tcp::<_, 1>::new(&stack, &tcpbuf);
        match get_ws_socket(&mut tcp, &mut rng, &mut buf, &mut nonce).await {
            Ok((socket, buf)) => {
                if let Err(_) = communicate_ws(socket, buf, &mut rng).await {
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

async fn get_ws_socket<'d, D>(
    tcp: &'d mut edge_nal_embassy::Tcp<'d, D, 1>,
    rng: &mut SmallRng,
    buf: &'d mut [u8],
    nonce: &'d mut [u8; NONCE_LEN],
) -> Result<(TcpSocket<'d, 1, 1024, 1024>, &'d mut [u8]), Error>
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

    return Ok(conn.release());
}

async fn communicate_ws<'d>(
    mut socket: TcpSocket<'d, 1, 1024, 1024>,
    buf: &mut [u8],
    rng: &mut SmallRng,
) -> Result<(), Error> {
    info!("pixelblaze: Connection upgraded to WS, starting traffic now");

    // Sending data

    let header = FrameHeader {
        frame_type: FrameType::Text(false),
        payload_len: PIXELBLAZE_MSG_GET_CONFIG.as_bytes().len() as _,
        mask_key: rng.next_u32().into(),
    };

    info!(
        "Sending text frame with payload \"{}\"",
        PIXELBLAZE_MSG_GET_CONFIG
    );
    header.send(&mut socket).await?;
    header
        .send_payload(&mut socket, PIXELBLAZE_MSG_GET_CONFIG.as_bytes())
        .await?;

    // receive loop
    loop {
        let header = FrameHeader::recv(&mut socket).await?;
        let payload = header.recv_payload(&mut socket, buf).await?;

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
                info!(
                    "pixelblaze: Got binary frame fragmented={}, message_type={}, payload={}",
                    fragmented,
                    PixelblazeMessageType::from(payload[0]),
                    payload,
                );
            }
            FrameType::Ping => {
                info!("pixelblaze: Got ping frame",);
                let header = FrameHeader {
                    frame_type: FrameType::Pong,
                    payload_len: 0,
                    mask_key: rng.next_u32().into(),
                };

                info!("pixelblaze: Sending pong");
                header.send(&mut socket).await?;
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
                    mask_key: rng.next_u32().into(),
                };

                info!("pixelblaze: Closing");

                header.send(&mut socket).await?;

                drop(socket);

                warn!("pixelblaze: closed");
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
