use std::path::Path;
use std::process::Stdio;

use agent_threads::control::{Action, Call, Reply};
use agent_threads::git::read::{Read, Request};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};

use super::{REPOSITORY, THREAD, fixture::bounded};
use crate::threads::tree::identity::Process;
use crate::threads::wire::GitReadRoot;

pub(super) struct Actor {
    child: Child,
    output: Lines<BufReader<ChildStdout>>,
    pub(super) root: GitReadRoot,
}

impl Actor {
    pub(super) fn start(path: &Path, generation: u64) -> Self {
        let mut child = Command::new("/usr/bin/perl")
            .args(["-e", include_str!("actor.pl")])
            .arg(path)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("synthetic agent");
        let pid = child.id().expect("live process");
        let process = Process::open(pid).expect("pinned synthetic child");
        let output = BufReader::new(child.stdout.take().expect("actor stdout")).lines();
        let root = GitReadRoot {
            thread: THREAD.to_string(),
            process: process.identity(),
            generation,
        };
        Self {
            child,
            output,
            root,
        }
    }

    pub(super) async fn prompt(&mut self, bytes: &str) {
        bounded(
            self.child
                .stdin
                .as_mut()
                .expect("actor stdin")
                .write_all(bytes.as_bytes()),
        )
        .await
        .expect("drive synthetic peer");
    }

    pub(super) async fn line(&mut self) -> String {
        bounded(self.output.next_line())
            .await
            .expect("peer output")
            .expect("peer still running")
    }

    pub(super) async fn request(&mut self, read: Read) {
        self.prompt("connect\n").await;
        assert_eq!(self.line().await, "connected");
        let mut frame = serde_json::to_string(&Call {
            id: 1,
            action: Action::GitRead(Request {
                repository: REPOSITORY.to_owned(),
                read,
            }),
        })
        .expect("Git request");
        frame.push('\n');
        self.prompt(&frame).await;
    }

    pub(super) async fn replies(&mut self) -> Vec<Reply> {
        self.prompt("read\n").await;
        let mut replies = Vec::new();
        loop {
            let line = self.line().await;
            if line == "eof" {
                return replies;
            }
            replies.push(serde_json::from_str(&line).expect("a runner reply"));
        }
    }
}
