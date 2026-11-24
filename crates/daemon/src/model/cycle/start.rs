//! `server_start` (Section 6): the launch of a `llama_server` in router mode with an empty preset.

use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::command;
use daemon_core::model_work::names::{Runtime, Word as _};
use daemon_core::model_work::releases::{self, Release};
use daemon_core::model_work::settings::ServerSettings;
use uuid::Uuid;

use super::{Node, PRESET, TEMPORARY};
use crate::model::install;
use crate::model::server::{self, Launch};
use crate::model::supervise::{Server, ServerState, Servers};

fn api_key() -> Option<zeroize::Zeroizing<String>> {
    let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut()).ok()?;
    Some(zeroize::Zeroizing::new(crate::model::fetch::hex(
        bytes.as_ref(),
    )))
}

impl Node {
    fn server_for(
        &self,
        id: Uuid,
        release: &Release,
        settings: &ServerSettings,
    ) -> Result<Server, Cause> {
        if self.settings.sandbox {
            // TODO(L1-6 follow-up): the sandbox profiles are not built, so with the setting on no server starts (Section 9).
            return Err(Cause::IsolationFailed);
        }
        releases::accepted(release)?;
        let entry = self
            .sources
            .find(release, self.os.as_str(), self.architecture.as_str())?;
        if !install::installed(&self.folders, release, entry)
            || release.runtime != Runtime::LlamaServer
        {
            return Err(Cause::RuntimeNotInstalled);
        }
        let program = self.folders.release(release).join(&entry.program);
        let bin = program.parent().unwrap_or(Path::new("/")).to_path_buf();
        let place = self.folders.server(id);
        let state = self.folders.state.clone();
        let launcher = self.launcher.clone();
        let settings = settings.clone();
        let ports = self.ports.clone();
        let launch = move || {
            let port = server::free_port(ports.clone())?;
            let key = api_key()?;
            let temporary = place.join(TEMPORARY);
            Some((
                Launch {
                    program: program.clone(),
                    arguments: command::llama_arguments(&settings, port, &place.join(PRESET)),
                    environment: command::environment(&temporary, &bin, &key),
                    state: state.clone(),
                    temporary,
                    ring: place.join("output.log"),
                    key,
                    place: place.clone(),
                    server: id,
                    launcher: launcher.clone(),
                },
                port,
            ))
        };
        Ok(Server {
            runtime: release.runtime.as_str(),
            launch: Box::new(launch),
            running: None,
            record: daemon_core::model_work::supervision::Supervision::default(),
            state: ServerState::Starting,
            since: 0,
            restart_at: None,
        })
    }

    /// `server_start`: the server starts again with an empty preset, so it holds no copy (Section 11.1).
    pub(super) async fn start(
        &self,
        server: Uuid,
        release: &Release,
        settings: &ServerSettings,
    ) -> Result<(), Cause> {
        crate::model::folders::make_all(&self.folders).map_err(|_| Cause::LimitDisk)?;
        let started = self.server_for(server, release, settings)?;
        let clock = self.started;
        self.servers.lock().await.stop(server).await;
        self.loads
            .loads
            .lock()
            .await
            .server_started(server, crate::model::load::now());
        let place = self.folders.server(server);
        crate::model::folders::write_private(&place, "", PRESET, b"version = 1\n")
            .map_err(|_| Cause::LimitDisk)?;
        let ready = Servers::ready(started, move || clock.elapsed().as_secs()).await?;
        self.servers.lock().await.map.insert(server, ready);
        Ok(())
    }
}
