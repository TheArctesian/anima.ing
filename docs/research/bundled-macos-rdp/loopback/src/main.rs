use ironrdp_server::{
    BitmapUpdate, Credentials, DesktopSize, DisplayUpdate, KeyboardEvent, MouseEvent, PixelFormat,
    RdpServer, RdpServerDisplay, RdpServerDisplayUpdates, RdpServerInputHandler, TlsIdentityCtx,
};
use std::{
    net::SocketAddr,
    num::{NonZeroU16, NonZeroUsize},
    path::Path,
};

struct SyntheticInput;
impl RdpServerInputHandler for SyntheticInput {
    fn keyboard(&mut self, event: KeyboardEvent) {
        println!("Synthetic keyboard event: {event:?}");
    }

    fn mouse(&mut self, event: MouseEvent) {
        println!("Synthetic pointer event: {event:?}");
    }
}

struct SyntheticDisplay;
struct Updates(bool);

#[async_trait::async_trait]
impl RdpServerDisplay for SyntheticDisplay {
    async fn size(&mut self) -> DesktopSize {
        DesktopSize {
            width: 32,
            height: 32,
        }
    }

    async fn updates(&mut self) -> anyhow::Result<Box<dyn RdpServerDisplayUpdates>> {
        Ok(Box::new(Updates(false)))
    }
}

#[async_trait::async_trait]
impl RdpServerDisplayUpdates for Updates {
    async fn next_update(&mut self) -> anyhow::Result<Option<DisplayUpdate>> {
        if self.0 {
            return std::future::pending().await;
        }
        self.0 = true;
        let side = NonZeroU16::new(32).unwrap();
        Ok(Some(DisplayUpdate::Bitmap(BitmapUpdate {
            x: 0,
            y: 0,
            width: side,
            height: side,
            format: PixelFormat::BgrA32,
            data: std::fs::read("synthetic-window.bgra")?.into(),
            stride: NonZeroUsize::new(32 * 4).unwrap(),
        })))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::args()
        .nth(1)
        .ok_or("missing localhost port")?
        .parse()?;
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let identity = TlsIdentityCtx::init_from_paths(Path::new("cert.pem"), Path::new("key.pem"))?;
    let mut server = RdpServer::builder()
        .with_addr(address)
        .with_hybrid(identity.make_acceptor()?, identity.pub_key)
        .with_input_handler(SyntheticInput)
        .with_display_handler(SyntheticDisplay)
        .build();
    server.set_credentials(Some(Credentials {
        username: "synthetic-probe".to_string(),
        password: std::env::var("ANIMA_RDP_PROBE_PASSWORD")?,
        domain: None,
    }));
    server.run().await?;
    Ok(())
}
