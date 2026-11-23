//! Route streams and UDP flows (HOST-050).

use std::{
    collections::BTreeMap,
    io,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _, DuplexStream, duplex},
    net::{TcpListener, TcpStream, UdpSocket},
};
use uuid::Uuid;

use crate::{
    hub::Hub,
    limits::Network,
    serve::Proxy,
    settings::{self, SettingsError},
    streams::{
        datagrams::{pump, read_frame, write_frame},
        listeners::Places,
        udp::may_answer,
    },
    wire::{Counts, Dial, Reach, StreamRoute, Table, Transport, WireError},
};

#[derive(Default)]
struct EchoHub {
    dials: Mutex<Vec<Dial>>,
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the trait gives each answer as a future; the double answers at once"
)]
impl Hub for EchoHub {
    type Stream = DuplexStream;

    async fn table(&self) -> Result<Table, WireError> {
        Ok(Table::default())
    }

    async fn connect(&self, dial: Dial) -> io::Result<DuplexStream> {
        self.dials.lock().unwrap().push(dial);
        let (hub, mut machine) = duplex(1024);
        tokio::spawn(async move {
            let mut buffer = [0_u8; 64];
            while let Ok(read) = machine.read(&mut buffer).await {
                if read == 0 || machine.write_all(&buffer[..read]).await.is_err() {
                    break;
                }
            }
        });
        Ok(hub)
    }

    async fn count(&self, _name: &str, _counts: Counts) -> Result<(), WireError> {
        Ok(())
    }
}

fn settings(pairs: &[(&str, &str)]) -> Result<Option<settings::Settings>, SettingsError> {
    let mut env: BTreeMap<String, String> = [
        ("ANIMA_ROUTE_PROXY_LISTEN", "127.0.0.1:443"),
        ("ANIMA_ROUTE_PROXY_HUB_SOCKET", "/run/anima/routes.sock"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect();
    for (name, value) in pairs {
        env.insert((*name).to_owned(), (*value).to_owned());
    }
    settings::from_env(&env)
}

fn stream(reach: Reach, transport: Transport, listen_port: u16) -> StreamRoute {
    StreamRoute {
        name: "mail.example.com".to_owned(),
        application: Uuid::nil(),
        transport,
        listen_port,
        reach,
    }
}

async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn a_tcp_stream_reaches_its_route_through_the_hub_by_its_name() {
    let settings = settings(&[]).unwrap().unwrap();
    let proxy = Arc::new(Proxy::new(EchoHub::default(), &settings).unwrap());
    let port = free_port().await;
    let places = Places {
        public: "127.0.0.1".parse().unwrap(),
        private: None,
        door: false,
    };
    let listeners = crate::streams::listeners::Listeners::default();
    listeners
        .keep(
            &proxy,
            places,
            vec![stream(Reach::Public, Transport::Tcp, port)],
        )
        .await;
    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    client.write_all(b"a1 LOGIN").await.unwrap();
    let mut echoed = [0_u8; 8];
    client.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"a1 LOGIN");
    let dial = proxy.hub().dials.lock().unwrap()[0].clone();
    assert_eq!(dial.name, "mail.example.com");
    assert_eq!(dial.transport, Transport::Tcp);
}

#[test]
fn a_private_stream_listens_only_on_the_private_address_and_none_in_door_mode() {
    let public: IpAddr = "203.0.113.4".parse().unwrap();
    let private: IpAddr = "100.64.0.9".parse().unwrap();
    let places = Places {
        public,
        private: None,
        door: false,
    };
    assert_eq!(
        places.of(&stream(Reach::Private, Transport::Tcp, 20_993)),
        None
    );
    assert_eq!(
        places.of(&stream(Reach::Public, Transport::Tcp, 20_993)),
        Some(SocketAddr::new(public, 20_993))
    );
    let places = Places {
        private: Some(private),
        ..places
    };
    assert_eq!(
        places.of(&stream(Reach::Private, Transport::Udp, 21_194)),
        Some(SocketAddr::new(private, 21_194))
    );
    let door = Places {
        door: true,
        ..places
    };
    assert_eq!(
        door.of(&stream(Reach::Public, Transport::Tcp, 20_993)),
        None
    );
}

#[test]
fn a_private_route_answers_only_the_nodes_of_the_network_of_anima() {
    let settings = settings(&[
        ("ANIMA_ROUTE_PROXY_PRIVATE_ADDRESS", "100.64.0.1"),
        (
            "ANIMA_ROUTE_PROXY_PRIVATE_NETWORKS",
            "100.64.0.0/10, fd7a:115c:a1e0::/48",
        ),
    ])
    .unwrap()
    .unwrap();
    let proxy = Proxy::new(EchoHub::default(), &settings).unwrap();
    assert!(proxy.reaches(Reach::Private, "100.100.1.2".parse().unwrap()));
    assert!(proxy.reaches(Reach::Private, "fd7a:115c:a1e0::5".parse().unwrap()));
    assert!(!proxy.reaches(Reach::Private, "192.168.1.20".parse().unwrap()));
    assert!(proxy.reaches(Reach::Public, "192.168.1.20".parse().unwrap()));
    let bare = Proxy::new(EchoHub::default(), &self::settings(&[]).unwrap().unwrap()).unwrap();
    assert!(!bare.reaches(Reach::Private, "100.100.1.2".parse().unwrap()));
    assert_eq!(
        self::settings(&[("ANIMA_ROUTE_PROXY_PRIVATE_ADDRESS", "100.64.0.1")]),
        Err(SettingsError::NoPrivateNetworks)
    );
    assert!("10.0.0.0/33".parse::<Network>().is_err());
}

#[test]
fn r_391_a_private_place_never_answers_the_internet() {
    let networks = ("ANIMA_ROUTE_PROXY_PRIVATE_NETWORKS", "100.64.0.0/10");
    for wide in ["0.0.0.0/0", "100.64.0.0/10, 0.0.0.0/4", "::/0", "fd7a::/8"] {
        assert_eq!(
            self::settings(&[
                ("ANIMA_ROUTE_PROXY_PRIVATE_ADDRESS", "100.64.0.1"),
                ("ANIMA_ROUTE_PROXY_PRIVATE_NETWORKS", wide),
            ]),
            Err(SettingsError::WidePrivateNetwork),
            "{wide}"
        );
    }
    for public in ["127.0.0.1", "0.0.0.0", "::"] {
        assert_eq!(
            self::settings(&[("ANIMA_ROUTE_PROXY_PRIVATE_ADDRESS", public), networks]),
            Err(SettingsError::PrivateAddressPublic),
            "{public}"
        );
    }
}

#[test]
fn a_flow_answers_a_source_with_no_more_bytes_than_it_sent() {
    assert!(may_answer(100, 0, 100));
    assert!(
        !may_answer(100, 60, 50),
        "a false source cannot be made a target"
    );
    assert!(!may_answer(0, 0, 1));
}

#[tokio::test]
async fn datagrams_go_as_frames_both_ways_and_end_when_idle() {
    let machine = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let hub = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    hub.connect(machine.local_addr().unwrap()).await.unwrap();
    let (mut proxy_side, mut hub_side) = duplex(1 << 17);
    let pumping = tokio::spawn(async move {
        pump(&mut hub_side, &hub, Duration::from_millis(200))
            .await
            .unwrap()
    });
    write_frame(&mut proxy_side, b"ping").await.unwrap();
    let mut datagram = [0_u8; 16];
    let (read, from) = machine.recv_from(&mut datagram).await.unwrap();
    assert_eq!(&datagram[..read], b"ping");
    machine.send_to(b"pong!", from).await.unwrap();
    let mut frame = Vec::new();
    let length = read_frame(&mut proxy_side, &mut frame)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&frame[..length], b"pong!");
    assert_eq!(
        pumping.await.unwrap(),
        (4, 5),
        "the pump ends after its idle time"
    );
}
