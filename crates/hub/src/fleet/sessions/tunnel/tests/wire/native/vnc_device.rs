use std::net::SocketAddr;

use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

pub(super) const WIDTH: u16 = 64;
pub(super) const HEIGHT: u16 = 48;

pub(super) struct VncDevice {
    pub(super) address: SocketAddr,
    pub(super) keys: mpsc::UnboundedReceiver<(u32, bool)>,
}

pub(super) async fn vnc_device(accepts: bool) -> VncDevice {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (heard, keys) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        // The gateway leaving first is not the device's fault.
        let _ = serve(client, accepts, heard).await;
    });
    VncDevice { address, keys }
}

async fn serve(
    mut client: TcpStream,
    accepts: bool,
    heard: mpsc::UnboundedSender<(u32, bool)>,
) -> std::io::Result<()> {
    if !signed_in(&mut client, accepts).await? {
        return Ok(());
    }
    loop {
        let kind = client.read_u8().await?;
        hear(&mut client, kind, &heard).await?;
    }
}

async fn signed_in(client: &mut TcpStream, accepts: bool) -> std::io::Result<bool> {
    client.write_all(b"RFB 003.008\n").await?;
    client.read_exact(&mut [0; 12]).await?;
    // One security type: VNC authentication.
    client.write_all(&[1, 2]).await?;
    client.read_u8().await?;
    client.write_all(&[7; 16]).await?;
    client.read_exact(&mut [0; 16]).await?;
    if !accepts {
        client.write_all(&[0, 0, 0, 1, 0, 0, 0, 0]).await?;
        return Ok(false);
    }
    client.write_all(&[0, 0, 0, 0]).await?;
    client.read_u8().await?;
    client
        .write_all(&[WIDTH.to_be_bytes(), HEIGHT.to_be_bytes()].concat())
        .await?;
    client.write_all(&[0; 20]).await?;
    Ok(true)
}

/// The first update request, for the whole screen, is answered with one red Raw rectangle.
pub(super) async fn hear(
    client: &mut (impl AsyncRead + AsyncWrite + Unpin),
    kind: u8,
    heard: &mpsc::UnboundedSender<(u32, bool)>,
) -> std::io::Result<()> {
    let length = match kind {
        0 => 19,
        2 => 3,
        3 => 9,
        4 => 7,
        5 => 5,
        _ => 0,
    };
    let mut body = vec![0; length];
    client.read_exact(&mut body).await?;
    match kind {
        2 => {
            let count = usize::from(u16::from_be_bytes([body[1], body[2]]));
            client.read_exact(&mut vec![0; count * 4]).await?;
        }
        3 if body[0] == 0 => {
            let size = [WIDTH.to_be_bytes(), HEIGHT.to_be_bytes()].concat();
            let mut update = [&[0, 0, 0, 1, 0, 0, 0, 0][..], &size, &[0, 0, 0, 0]].concat();
            update.extend([0, 0, 200, 0].repeat(usize::from(WIDTH) * usize::from(HEIGHT)));
            client.write_all(&update).await?;
        }
        4 => {
            let keysym = u32::from_be_bytes([body[3], body[4], body[5], body[6]]);
            let _ = heard.send((keysym, body[0] == 1));
        }
        _ => {}
    }
    Ok(())
}
