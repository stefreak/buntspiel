use defmt::{info, warn};
use embassy_net::{tcp::TcpSocket, IpEndpoint, Ipv4Address};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;

#[embassy_executor::task]
pub(crate) async fn pixelblaze_websocket(
    stack: &'static embassy_net::Stack<cyw43::NetDriver<'static>>,
) -> ! {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        info!("Connecting to Pixelblaze http://192.168.4.1:81...");
        if let Err(e) = socket
            .connect(IpEndpoint::new(Ipv4Address::new(192, 168, 4, 1).into(), 81))
            .await
        {
            warn!("connect error: {:?}. Trying again in 5 seconds", e);
            Timer::after(Duration::from_secs(5)).await;
            continue;
        }

        info!("Received connection from {:?}", socket.remote_endpoint());

        loop {
            match socket.write_all("hello".as_bytes()).await {
                Ok(()) => {}
                Err(e) => {
                    warn!("write error: {:?}", e);
                    break;
                }
            };

            Timer::after(Duration::from_millis(1000)).await;
        }
    }
}
