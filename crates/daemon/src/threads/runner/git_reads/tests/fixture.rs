use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_threads::control::Action;
use agent_threads::git::read::Outcome as ReadOutcome;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{UnixListener, UnixStream};

use super::super::super::{Runner, socket};
use super::super::Completed;
use super::{actor::Actor, setup::make_runner};
use crate::threads::holder::SOCKET;
use crate::threads::link::HolderLink;
use crate::threads::tree::identity::Process;
use crate::threads::wire::{self, Event, GitReadRoot, Held};

pub(super) async fn bounded<T>(go: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), go)
        .await
        .expect("fixture operation finished")
}

pub(super) fn capable() -> bool {
    let (socket, _peer) = UnixStream::pair().expect("synthetic sockets");
    let creds = socket.peer_cred().expect("socket credentials");
    let capable = Process::peer(&socket, creds.pid()).is_some();
    if !capable {
        assert!(
            std::env::var_os("ANIMA_REQUIRE_GIT_READS").is_none(),
            "positive W3 coverage requires socket-bound pidfs"
        );
        eprintln!("Skipping positive W3 fixture: socket-bound pidfs is unavailable.");
    }
    capable
}

pub(super) struct Fixture {
    pub(super) runner: Runner,
    pub(super) holder: Option<UnixStream>,
    pub(super) peer: Actor,
    pub(super) path: PathBuf,
    incoming: tokio::sync::mpsc::Receiver<socket::Incoming>,
    accept: tokio::task::JoinHandle<()>,
    _home: tempfile::TempDir,
}

impl Fixture {
    pub(super) async fn open() -> Self {
        let home = tempfile::tempdir_in("/var/tmp").expect("private synthetic home");
        let dir = home.path().join("threads");
        super::super::super::record::private_folder(&dir).expect("threads folder");
        let socket = socket::bind(&dir).expect("public runner socket");
        let path = dir.join(agent_threads::control::SOCKET);
        let (send, incoming) = tokio::sync::mpsc::channel(8);
        let accept = tokio::spawn(socket::accept(socket, send, dir.clone()));
        let peer = Actor::start(&path, 1);
        let (link, mut holder) = holder_link(&dir).await;
        let mut runner = make_runner(home.path(), dir, link).await;
        hello(&mut runner, &mut holder, &peer.root).await;
        Self {
            runner,
            holder: Some(holder),
            peer,
            path,
            incoming,
            accept,
            _home: home,
        }
    }

    pub(super) async fn pending(
        &mut self,
        read: agent_threads::git::read::Read,
    ) -> (socket::Client, Completed) {
        self.peer.request(read).await;
        let incoming = bounded(self.incoming.recv())
            .await
            .expect("public socket call");
        let Action::GitRead(request) = incoming.call.action else {
            panic!("Git read expected")
        };
        let client = incoming.client;
        self.runner.git_read(
            client.reply_to(incoming.call.id),
            incoming.git_peer,
            request,
        );
        let bound = summary_step(&mut self.runner).await;
        self.runner
            .completed(super::super::super::completed::Completed::GitRead(bound));
        let completion = summary_step(&mut self.runner).await;
        assert!(
            matches!(
                completion.result,
                super::super::Finished::Summary(
                    _,
                    ReadOutcome::Status { .. } | ReadOutcome::Checkpoints { .. }
                )
            ),
            "a real worker produced a summary"
        );
        (client, completion)
    }

    pub(super) async fn send(&mut self, event: Event) {
        publish(self.holder.as_mut().expect("holder connected"), &event).await;
        let received = bounded(self.runner.link.events.recv())
            .await
            .expect("holder event");
        assert_eq!(received, event);
        self.runner.holder_event(received).await;
    }

    pub(super) async fn disconnect_holder(&mut self) {
        drop(self.holder.take());
        assert!(bounded(self.runner.link.events.recv()).await.is_none());
    }

    pub(super) async fn reconnect_holder(&mut self) {
        let (link, mut holder) = holder_link(&self.runner.dir).await;
        self.runner.link = link;
        hello(&mut self.runner, &mut holder, &self.peer.root).await;
        self.holder = Some(holder);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

async fn summary_step(runner: &mut Runner) -> Completed {
    bounded(
        runner
            .git_reads
            .receive
            .as_mut()
            .expect("runner queue")
            .recv(),
    )
    .await
    .expect("worker completed")
}

async fn holder_link(dir: &Path) -> (HolderLink, UnixStream) {
    let path = dir.join(SOCKET);
    if path.exists() {
        std::fs::remove_file(&path).expect("replace synthetic holder socket");
    }
    let listener = UnixListener::bind(&path).expect("synthetic holder endpoint");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("private holder endpoint");
    let link = HolderLink::connect(dir)
        .await
        .expect("production holder link");
    let (mut side, _) = bounded(listener.accept()).await.expect("holder connected");
    let mut line = String::new();
    bounded(BufReader::new(&mut side).read_line(&mut line))
        .await
        .expect("read Hello");
    let command: wire::Command = serde_json::from_str(&line).expect("Hello frame");
    assert!(matches!(
        command.action,
        wire::Action::Hello {
            git_reads: true,
            ..
        }
    ));
    (link, side)
}

async fn hello(runner: &mut Runner, holder: &mut UnixStream, root: &GitReadRoot) {
    publish(
        holder,
        &Event::Hello {
            version: wire::VERSION,
            build: None,
            threads: vec![Held {
                thread: root.thread.clone(),
                pid: root.process.pid,
                exit: None,
            }],
            git_read_roots: Some(vec![root.clone()]),
        },
    )
    .await;
    let event = bounded(runner.link.events.recv()).await.expect("Hello");
    runner.holder_event(event).await;
}

async fn publish(holder: &mut UnixStream, event: &Event) {
    bounded(holder.write_all(wire::frame(event).as_bytes()))
        .await
        .expect("send holder frame");
}
