//! The caps that keep slow clients from holding each stream of a link (SF-NEW-revroutelimits):
//! a TCP stream and a WebSocket after its `101` hold a place of their route until they end, a
//! write to a client that does not read ends at its time, and a UDP stream opens only so many new
//! flows each second and from each source network.

use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _, DuplexStream, duplex},
    net::{TcpListener, TcpStream, UdpSocket},
    time::{sleep, timeout},
};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    hub::Hub,
    relay::Idle,
    serve::{Proxy, https::answer},
    settings::{self, Settings},
    streams::listeners::{Listeners, Places},
    wire::{Counts, Dial, Named, Reach, StreamRoute, Table, Transport, WireError},
};

const NAME: &str = "ws.example.com";

/// A hub with one name, whose machine answers `101` to the head of a request and then echoes;
/// the machine of each stream only echoes.
#[derive(Default)]
struct UpgradeHub {
    dials: Mutex<Vec<Dial>>,
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the trait gives each answer as a future; the double answers at once"
)]
impl Hub for UpgradeHub {
    type Stream = DuplexStream;

    async fn table(&self) -> Result<Table, WireError> {
        let key = rcgen::KeyPair::generate().unwrap();
        let params = rcgen::CertificateParams::new(vec![NAME.to_owned()]).unwrap();
        let named = Named {
            name: NAME.to_owned(),
            application: Uuid::nil(),
            chain_pem: params.self_signed(&key).unwrap().pem(),
            key_pem: Zeroizing::new(key.serialize_pem()),
            reach: Reach::Public,
            redirect_to: None,
        };
        Ok(Table {
            names: vec![named],
            ..Table::default()
        })
    }

    async fn connect(&self, dial: Dial) -> io::Result<DuplexStream> {
        let upgrade = dial.name == NAME;
        self.dials.lock().unwrap().push(dial);
        let (hub, machine) = duplex(1024);
        tokio::spawn(machine_of(machine, upgrade));
        Ok(hub)
    }

    async fn count(&self, _name: &str, _counts: Counts) -> Result<(), WireError> {
        Ok(())
    }
}

/// A machine that, for an upgrade, answers `101` to the head of the request; then it echoes.
async fn machine_of(mut machine: DuplexStream, upgrade: bool) {
    let mut buffer = [0_u8; 1024];
    if upgrade {
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let Ok(read @ 1..) = machine.read(&mut buffer).await else {
                return;
            };
            head.extend_from_slice(&buffer[..read]);
        }
        let switching = b"HTTP/1.1 101 Switching Protocols\r\n\r\n";
        if machine.write_all(switching).await.is_err() {
            return;
        }
    }
    while let Ok(read @ 1..) = machine.read(&mut buffer).await {
        if machine.write_all(&buffer[..read]).await.is_err() {
            break;
        }
    }
}

fn settings(pairs: &[(&str, &str)]) -> Settings {
    let mut env: BTreeMap<String, String> = [
        ("ANIMA_ROUTE_PROXY_LISTEN", "127.0.0.1:443"),
        ("ANIMA_ROUTE_PROXY_HUB_SOCKET", "/run/anima/routes.sock"),
        ("ANIMA_ROUTE_PROXY_OPEN_PER_CLIENT", "64"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect();
    for (name, value) in pairs {
        env.insert((*name).to_owned(), (*value).to_owned());
    }
    settings::from_env(&env).unwrap().unwrap()
}

/// A proxy whose streams listen on the loopback address, with `pairs` as its settings.
fn proxy(pairs: &[(&str, &str)]) -> Arc<Proxy<UpgradeHub>> {
    Arc::new(Proxy::new(UpgradeHub::default(), &settings(pairs)).unwrap())
}

const LOOPBACK: Places = Places {
    public: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
    private: None,
    door: false,
};

fn stream(transport: Transport, listen_port: u16) -> StreamRoute {
    StreamRoute {
        name: "mail.example.com".to_owned(),
        application: Uuid::nil(),
        transport,
        listen_port,
        reach: Reach::Public,
    }
}

/// Whether a line through `client` comes back.
async fn echoes(client: &mut TcpStream) -> bool {
    if client.write_all(b"a1 NOOP\r\n").await.is_err() {
        return false;
    }
    let mut echoed = [0_u8; 9];
    matches!(
        timeout(Duration::from_secs(2), client.read_exact(&mut echoed)).await,
        Ok(Ok(_))
    )
}

#[tokio::test]
async fn a_tcp_stream_holds_a_place_of_its_route_until_it_ends() {
    let proxy = proxy(&[("ANIMA_ROUTE_PROXY_CONNECTIONS", "2")]);
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let listeners = Listeners::default();
    listeners
        .keep(&proxy, LOOPBACK, vec![stream(Transport::Tcp, port)])
        .await;
    let mut first = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut second = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    assert!(echoes(&mut first).await && echoes(&mut second).await);
    let mut third = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    assert!(!echoes(&mut third).await, "the route is at its cap");
    drop(first);
    sleep(Duration::from_millis(200)).await;
    let mut fourth = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    assert!(
        echoes(&mut fourth).await,
        "the place of the first is free again"
    );
}

#[tokio::test]
async fn a_websocket_after_its_101_holds_a_place_of_its_route() {
    let proxy = proxy(&[("ANIMA_ROUTE_PROXY_CONNECTIONS", "1")]);
    assert_eq!(proxy.refresh().await.unwrap().0, 1);
    let client = "203.0.113.7".parse().unwrap();
    let (mut socket, side) = duplex(4096);
    let held = Arc::clone(&proxy);
    let serving =
        tokio::spawn(async move { answer(&held, side, (NAME, client), Reach::Public).await });
    socket
        .write_all(b"GET /ws HTTP/1.1\r\nhost: ws.example.com\r\nconnection: upgrade\r\nupgrade: websocket\r\nsec-websocket-key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n")
        .await
        .unwrap();
    let mut switching = [0_u8; 12];
    socket.read_exact(&mut switching).await.unwrap();
    assert_eq!(&switching, b"HTTP/1.1 101");
    let (mut other, other_side) = duplex(4096);
    answer(&proxy, other_side, (NAME, client), Reach::Public).await;
    let mut refused = Vec::new();
    other.read_to_end(&mut refused).await.unwrap();
    assert!(
        refused.starts_with(b"HTTP/1.1 503"),
        "the WebSocket holds the one place"
    );
    drop(socket);
    timeout(Duration::from_secs(2), serving)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn a_write_to_a_client_that_does_not_read_ends_at_its_time() {
    let (_not_reading, side) = duplex(64);
    let mut client =
        Idle::new(side, Duration::from_secs(3600)).writes_within(Duration::from_millis(100));
    let error = timeout(Duration::from_secs(2), client.write_all(&[0_u8; 4096]))
        .await
        .expect("the write ends at its time, not at the idle time")
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn a_udp_stream_opens_only_its_new_flows_each_second() {
    let proxy = proxy(&[("ANIMA_ROUTE_PROXY_UDP_NEW_PER_SECOND", "1")]);
    let port = UdpSocket::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let listeners = Listeners::default();
    listeners
        .keep(&proxy, LOOPBACK, vec![stream(Transport::Udp, port)])
        .await;
    let mut sources = Vec::new();
    for _ in 0..6 {
        let source = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        source.send_to(b"ping", ("127.0.0.1", port)).await.unwrap();
        sources.push(source);
    }
    sleep(Duration::from_millis(300)).await;
    let dials = proxy.hub().dials.lock().unwrap().len();
    assert_eq!(
        dials, 2,
        "a burst of twice the rate, then no new flow until it fills again"
    );
}

#[tokio::test]
async fn a_udp_stream_opens_only_its_flows_of_one_source_network() {
    let proxy = proxy(&[
        ("ANIMA_ROUTE_PROXY_UDP_NEW_PER_SECOND", "100"),
        ("ANIMA_ROUTE_PROXY_UDP_FLOWS_PER_NETWORK", "3"),
    ]);
    let port = UdpSocket::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let listeners = Listeners::default();
    listeners
        .keep(&proxy, LOOPBACK, vec![stream(Transport::Udp, port)])
        .await;
    let mut sources = Vec::new();
    for _ in 0..6 {
        let source = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        source.send_to(b"ping", ("127.0.0.1", port)).await.unwrap();
        sources.push(source);
    }
    sleep(Duration::from_millis(300)).await;
    let dials = proxy.hub().dials.lock().unwrap().len();
    assert_eq!(dials, 3, "each source of 127.0.0.0/24 is one network");
}
