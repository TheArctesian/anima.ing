//! The persistent private link of a standalone CLI. An existing app-owned unit is never replaced.
use super::device::ended;
use crate::{
    failure::Failure,
    hub::{Origin, account::Access},
    private_file,
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncWriteExt as _, process::Command};

pub(super) struct RemoteLink {
    dir: PathBuf,
    program: PathBuf,
    installed: bool,
}

impl RemoteLink {
    pub(super) async fn prepare(config: &Path) -> Result<Self, Failure> {
        if !cfg!(unix) {
            return Err(ended("Use the Anima app to sign in this Windows computer."));
        }
        #[cfg(unix)]
        if rustix::process::getuid().is_root() {
            return Err(ended("Install Anima as a dedicated user, not root."));
        }
        let program = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("anima-link")))
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                ended("Install Anima's CLI and private link together using the Anima installer.")
            })?;
        let dir = config.join("hosted");
        let mut link = Self {
            dir,
            program,
            installed: false,
        };
        link.check_unit().await?;
        Ok(link)
    }

    async fn check_unit(&mut self) -> Result<(), Failure> {
        let link = self;
        let shown = link.command(&["unit", "show"], None).await?;
        let shown: serde_json::Value = serde_json::from_slice(&shown)
            .map_err(|_| ended("The installed link could not be checked."))?;
        if shown["installed"].as_bool() == Some(true) {
            let file = shown["file"]
                .as_str()
                .ok_or_else(|| ended("The installed link has no unit path."))?;
            let contents = std::fs::read_to_string(file)
                .map_err(|_| ended("The installed link unit could not be read."))?;
            let state = link.dir.join("state");
            let socket = link.dir.join("link.sock");
            if !link.dir.join("installed").is_file()
                || !contents.contains(&state.display().to_string())
                || !contents.contains(&socket.display().to_string())
                || !contents.contains(&link.program.display().to_string())
            {
                return Err(ended(
                    "An existing Anima link manages this computer. Sign in through the Anima app, or keep using its installed CLI context.",
                ));
            }
            link.installed = true;
        }
        Ok(())
    }

    pub(super) async fn connect(mut self, access: &Access) -> Result<Origin, Failure> {
        self.check_unit().await?;
        self.join(access).await?;
        let state = self.dir.join("state");
        let state = state
            .to_str()
            .ok_or_else(|| ended("The link path is not text."))?;
        let socket = self.dir.join("link.sock");
        let socket = socket
            .to_str()
            .ok_or_else(|| ended("The socket path is not text."))?;
        let origin = Origin::parse(&format!("unix:{socket}"))
            .map_err(|_| ended("The link socket path is invalid."))?;
        if self.installed {
            crate::hub::account::wait_for_link(&origin).await?;
            return Ok(origin);
        }
        self.check_unit().await?;
        if self.installed {
            return Err(ended(
                "The link was installed while approval was pending. Retry sign-in to reuse it.",
            ));
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|_| ended("A local port could not be reserved."))?;
        let listen = listener
            .local_addr()
            .map_err(|_| ended("A local port could not be read."))?
            .to_string();
        drop(listener);
        private_file::write(&self.dir, "installed", b"1")
            .map_err(|_| ended("The link installation could not be recorded."))?;
        self.command(
            &[
                "unit",
                "install",
                "-state",
                state,
                "-listen",
                &listen,
                "-daemon-socket",
                socket,
            ],
            None,
        )
        .await?;
        private_file::write(&self.dir, "installed", b"1")
            .map_err(|_| ended("The link installation could not be recorded."))?;
        let origin = Origin::parse(&format!("unix:{socket}"))
            .map_err(|_| ended("The link socket path is invalid."))?;
        crate::hub::account::wait_for_link(&origin).await?;
        Ok(origin)
    }

    async fn join(&self, access: &Access) -> Result<(), Failure> {
        let binding = private_file::read(&self.dir.join("hub"))
            .map_err(|_| ended("The link's saved account could not be read."))?;
        if binding
            .as_ref()
            .is_some_and(|hub| hub.as_slice() != access.hub.as_bytes())
        {
            return Err(ended(
                "This computer's link belongs to another hub. Remove that link explicitly before changing accounts.",
            ));
        }
        if access.invite.is_some() && self.dir.join("joined").is_file() {
            return Err(ended(
                "This link is already joined. Approve using the same computer name used on its first sign-in.",
            ));
        }
        if access.invite.is_none() && !self.dir.join("joined").is_file() {
            return Err(ended(
                "That computer name already belongs to another link. Retry with --computer and a unique name.",
            ));
        }
        private_file::write(&self.dir, "hub", access.hub.as_bytes())
            .map_err(|_| ended("The private link directory could not be written."))?;
        let state = self.dir.join("state");
        let state = state
            .to_str()
            .ok_or_else(|| ended("The link path is not text."))?;
        if let Some(invite) = &access.invite {
            self.command(&["join", "-state", state], Some(invite))
                .await?;
            private_file::write(&self.dir, "joined", b"1")
                .map_err(|_| ended("The joined link could not be recorded."))?;
        }
        Ok(())
    }

    async fn command(&self, args: &[&str], input: Option<&str>) -> Result<Vec<u8>, Failure> {
        let mut child = Command::new(&self.program)
            .args(args)
            .kill_on_drop(true)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ended("The private link could not start."))?;
        if let Some(input) = input {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| ended("The link could not read its invitation."))?;
            stdin
                .write_all(input.as_bytes())
                .await
                .map_err(|_| ended("The link refused its invitation."))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|_| ended("The link refused its invitation."))?;
        }
        let result = tokio::time::timeout(Duration::from_secs(45), child.wait_with_output())
            .await
            .map_err(|_| ended("The private link timed out."))?
            .map_err(|_| ended("The private link did not finish."))?;
        if !result.status.success() {
            return Err(ended(
                "The private link refused setup. Check the user service manager and retry.",
            ));
        }
        Ok(result.stdout)
    }
}
