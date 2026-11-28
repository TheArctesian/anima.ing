//! A real K8 enrollment and Linux supervisor for the protected K10 tests.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anima_daemon::config::home::Home;
use axum::Router;
use tokio::{io::AsyncWriteExt as _, task::JoinHandle};

mod process;
mod tls;

use process::{Installation, Supervisor};
pub(super) use process::{Role, RoleProcess};

pub(super) const CODE: &str = "anima_enrol_AAAAAAAAAAAAAAAAAAAAAA";

pub(super) struct Protected {
    supervisor: Option<Supervisor>,
    installation: Installation,
    listener: Option<tls::Listener>,
    origin: String,
}

impl Protected {
    pub(super) fn new(person_home: &Path, state: &Path) -> Self {
        Home::at(state.to_owned()).ensure().expect("daemon home");
        let listener = tls::Listener::bind();
        let origin = listener.origin();
        let installation = Installation::new(person_home, state, &origin, listener.root());
        Self {
            supervisor: None,
            installation,
            listener: Some(listener),
            origin,
        }
    }

    pub(super) fn serve(&mut self, router: Router) -> JoinHandle<()> {
        let listener = self.listener.take().expect("the unstarted K8 listener");
        tokio::spawn(async move { axum::serve(listener, router).await.expect("the TLS hub") })
    }

    pub(super) async fn enrol(&self) {
        let mut child = tokio::process::Command::from(self.installation.command())
            .args(["enrol", &self.origin])
            .stdin(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("the K8 enrollment process starts");
        let mut stdin = child.stdin.take().expect("the enrollment code pipe");
        stdin
            .write_all(CODE.as_bytes())
            .await
            .expect("write the code");
        stdin.write_all(b"\n").await.expect("terminate the code");
        drop(stdin);
        let status = tokio::time::timeout(Duration::from_secs(20), child.wait())
            .await
            .expect("K8 enrollment finishes")
            .expect("the enrollment process returns a status");
        assert!(status.success(), "K8 enrollment failed: {status}");
    }

    pub(super) async fn start(&mut self) {
        assert!(
            self.supervisor.is_none(),
            "start one supervisor per fixture"
        );
        self.supervisor = Some(self.installation.start());
        self.process(Role::Daemon).await;
        self.supervisor
            .as_ref()
            .expect("the started supervisor")
            .assert_inherited(&self.inherited_descriptor_path());
    }

    pub(super) fn inherited_descriptor_path(&self) -> PathBuf {
        self.installation.inherited_descriptor_path()
    }

    pub(super) fn process_now(&self, role: Role) -> Option<RoleProcess> {
        self.supervisor.as_ref()?.process(role)
    }

    pub(super) async fn process(&self, role: Role) -> RoleProcess {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(process) = self.process_now(role) {
                    return process;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the protected {role:?} did not start"))
    }
}
