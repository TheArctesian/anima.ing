//! The device's SSH server on loopback, a `russh` server in the test (K4 Section 7).

mod device;
mod rounds;

use std::{
    net::Shutdown,
    sync::{Arc, Mutex},
};

use russh::{
    Channel, ChannelId, MethodKind, MethodSet,
    keys::{PrivateKey, ssh_key::private::Ed25519Keypair},
    server::{self, Msg},
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
};

use device::Device;

use crate::fleet::{sessions::tests::fixture::PASSWORD, trust::PublicKey};

/// The user `docker-host`'s SSH settings name in the devices fixture.
const USER: &str = "operator";
/// The hub can send this much before its write waits on held input.
const INPUT_WINDOW: u32 = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Heard {
    Auth(&'static str),
    Pty {
        term: String,
        cols: u32,
        rows: u32,
        pixels: (u32, u32),
        modes: usize,
    },
    Shell,
    WindowChange {
        cols: u32,
        rows: u32,
        pixels: (u32, u32),
    },
    Input(Vec<u8>),
    /// A request or channel the hub must never send (K4 Section 8.4), by its SSH name.
    Forbidden(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Prompts {
    /// One round of this many prompts.
    Round(usize),
    /// Rounds of one prompt, each answer earning another.
    Endless,
    /// This many rounds of one prompt; the last answer is accepted.
    Rounds(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hold {
    Nothing,
    SignIn,
    Pty,
    /// The client's input after the first packet; the small window makes the hub's write wait.
    Input,
}

#[derive(Debug, Default)]
pub(super) struct Ledger {
    pub(super) opened: Vec<(&'static str, ChannelId)>,
    pub(super) requested_on: Vec<ChannelId>,
    pub(super) closed: Vec<ChannelId>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Script {
    pub(super) password: &'static str,
    /// It offers `keyboard-interactive` only, and takes the password as the prompt's answer.
    pub(super) prompts_only: bool,
    pub(super) refuse_pty: bool,
    pub(super) refuse_shell: bool,
    pub(super) prompts: Prompts,
    pub(super) hold: Hold,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            password: PASSWORD,
            prompts_only: false,
            refuse_pty: false,
            refuse_shell: false,
            prompts: Prompts::Round(1),
            hold: Hold::Nothing,
        }
    }
}

pub(super) struct FakeSsh {
    pub(super) port: u16,
    pub(super) key: PublicKey,
    pub(super) heard: mpsc::UnboundedReceiver<Heard>,
    pub(super) ledger: Arc<Mutex<Ledger>>,
    shell: Arc<Mutex<Option<(server::Handle, ChannelId)>>>,
    socket: Arc<Mutex<Option<std::net::TcpStream>>>,
    gate: watch::Sender<bool>,
}

pub(super) async fn fake_ssh(seed: u8, script: Script) -> FakeSsh {
    let host_key = PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]));
    let key = PublicKey::from_blob(host_key.public_key().to_bytes().unwrap()).unwrap();
    let methods: &[MethodKind] = if script.prompts_only {
        &[MethodKind::KeyboardInteractive]
    } else {
        &[MethodKind::Password, MethodKind::PublicKey]
    };
    let window_size = if script.hold == Hold::Input {
        INPUT_WINDOW
    } else {
        server::Config::default().window_size
    };
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        methods: MethodSet::from(methods),
        auth_rejection_time: std::time::Duration::ZERO,
        window_size,
        ..server::Config::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (heard_tx, heard) = mpsc::unbounded_channel();
    let shell = Arc::new(Mutex::new(None));
    let socket = Arc::new(Mutex::new(None));
    let ledger = Arc::new(Mutex::new(Ledger::default()));
    let (gate, opened) = watch::channel(false);
    let (served_shell, served_socket) = (Arc::clone(&shell), Arc::clone(&socket));
    let served_ledger = Arc::clone(&ledger);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let twin = stream.into_std().unwrap();
            *served_socket.lock().unwrap() = Some(twin.try_clone().unwrap());
            let stream = tokio::net::TcpStream::from_std(twin).unwrap();
            let device = Device {
                script,
                heard: heard_tx.clone(),
                shell: Arc::clone(&served_shell),
                ledger: Arc::clone(&served_ledger),
                gate: opened.clone(),
                answered: 0,
            };
            let config = Arc::clone(&config);
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, stream, device).await {
                    let _ended = session.await;
                }
            });
        }
    });
    FakeSsh {
        port,
        key,
        heard,
        ledger,
        shell,
        socket,
        gate,
    }
}

impl FakeSsh {
    fn shell(&self) -> (server::Handle, ChannelId) {
        self.shell
            .lock()
            .unwrap()
            .clone()
            .expect("the shell is open")
    }

    pub(super) async fn say(&self, bytes: &[u8]) {
        self.try_say(bytes)
            .await
            .expect("the shell's output is read");
    }

    pub(super) async fn try_say(&self, bytes: &[u8]) -> Result<(), ()> {
        let (handle, channel) = self.shell();
        handle.data(channel, bytes.to_vec()).await.map_err(|_| ())
    }

    pub(super) async fn complain(&self, bytes: &[u8]) {
        let (handle, channel) = self.shell();
        handle
            .extended_data(channel, 1, bytes.to_vec())
            .await
            .unwrap();
    }

    pub(super) async fn exit(&self) {
        let (handle, channel) = self.shell();
        handle.exit_status_request(channel, 0).await.unwrap();
        handle.eof(channel).await.unwrap();
        handle.close(channel).await.unwrap();
    }

    /// The device's TCP connection goes away with no SSH disconnect, as a lost network does.
    pub(super) fn cut(&self) {
        let socket = self.socket.lock().unwrap().take().expect("a connection");
        socket.shutdown(Shutdown::Both).unwrap();
    }

    pub(super) fn release(&self) {
        self.gate.send_replace(true);
    }

    pub(super) async fn open_channels(&self) -> Vec<Result<Channel<Msg>, russh::Error>> {
        let (handle, _) = self.shell();
        vec![
            handle.channel_open_session().await,
            handle.channel_open_x11("127.0.0.1", 6010).await,
            handle.channel_open_agent().await,
            handle
                .channel_open_direct_tcpip("127.0.0.1", 22, "127.0.0.1", 50_000)
                .await,
            handle
                .channel_open_forwarded_tcpip("127.0.0.1", 8080, "127.0.0.1", 50_001)
                .await,
            handle.channel_open_direct_streamlocal("/run/agent").await,
            handle
                .channel_open_forwarded_streamlocal("/run/agent")
                .await,
        ]
    }

    pub(super) async fn open_x11(&self) -> bool {
        let (handle, _) = self.shell();
        handle.channel_open_x11("127.0.0.1", 6010).await.is_ok()
    }

    pub(super) async fn next(&mut self) -> Heard {
        tokio::time::timeout(super::WAIT, self.heard.recv())
            .await
            .expect("the server hears from the hub")
            .expect("the server is running")
    }

    pub(super) async fn next_after_sign_in(&mut self) -> Heard {
        loop {
            match self.next().await {
                Heard::Auth(_) => {}
                other => return other,
            }
        }
    }

    pub(super) fn heard_so_far(&mut self) -> Vec<Heard> {
        std::iter::from_fn(|| self.heard.try_recv().ok()).collect()
    }
}
