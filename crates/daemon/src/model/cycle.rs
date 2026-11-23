//! One cycle of model work (Section 3, CAG-002, CAG-003); the caller runs it only while the switch at the machine is on.

use std::path::{Path, PathBuf};
use std::time::Instant;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::command;
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::found::{self, Root};
use daemon_core::model_work::machine::MachineSettings;
use daemon_core::model_work::names::{Runtime, Word as _};
use daemon_core::model_work::order::{self, Checked, ModelMachine, Order, Parameters};
use daemon_core::model_work::releases::{self, Release, Sources};
use daemon_core::model_work::settings::{ServerSettings, SettingLimits};
use daemon_core::wire::{
    Claim, DaemonToken, Free, Switch, WorkArchitecture, WorkKind, WorkOs, WorkType,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::install::{self, Place};
use super::pull::watch::{self, Watched};
use super::server::{self, Launch};
use super::state;
use super::supervise::{Server, ServerState, Servers, Shared};
use crate::hub::HubClient;
use crate::hub::answer::{Answer, Cause as HubCause};

/// The folder of a server's temporary files, below its folder in the state folder.
pub const TEMPORARY: &str = "tmp";

pub const BUILT: [WorkType; 8] = [
    WorkType::ModelReport,
    WorkType::ModelDrain,
    WorkType::RuntimeInstall,
    WorkType::ModelPull,
    WorkType::CopyDelete,
    WorkType::CopyAdopt,
    WorkType::ServerStart,
    WorkType::ServerStop,
];

pub struct Node {
    pub device: Uuid,
    pub os: WorkOs,
    pub architecture: WorkArchitecture,
    pub folders: Folders,
    pub sources: Sources,
    pub settings: MachineSettings,
    pub servers: Shared,
    pub switch_on: bool,
    pub started: Instant,
    pub reserve: u64,
    /// The daemon program: the launcher of a runtime on Linux (R-331).
    pub launcher: PathBuf,
    pub ports: std::ops::RangeInclusive<u16>,
    /// The fixed folders of Section 10.2 for this installation (`found::tool_folders`).
    pub tool_folders: Vec<Root>,
}

/// The JSON report of attempt `attempt` of item `id` (K10 Section 7, with `model`).
#[must_use]
pub fn report(
    id: Uuid,
    attempt: u32,
    outcome: Result<Option<Value>, Cause>,
    seconds: u64,
) -> Value {
    let detail = outcome.as_ref().err().and_then(|cause| cause.detail());
    let (state, cause, model) = match outcome {
        Ok(model) => ("succeeded", None, model),
        Err(cause) if cause.is_refusal() => ("refused", Some(cause.as_str()), None),
        Err(cause) => ("failed", Some(cause.as_str()), None),
    };
    let mut result = json!({
        "outcome": state, "cause": cause, "seconds": seconds,
        "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" },
    });
    if let Some(model) = model {
        result["model"] = model;
    }
    if let Some(detail) = detail {
        result["detail"] = json!(detail);
    }
    json!({ "id": id, "attempt": attempt, "state": state, "result": result })
}

fn api_key() -> Option<zeroize::Zeroizing<String>> {
    let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut()).ok()?;
    Some(zeroize::Zeroizing::new(super::fetch::hex(bytes.as_ref())))
}

impl Node {
    /// Ends each server of an earlier run (R-331); the daemon calls it once at start, before any claim.
    ///
    /// # Errors
    /// An I/O error of the state folder.
    pub fn sweep(&self) -> std::io::Result<usize> {
        super::tree::sweep(&self.folders.state.join("servers"))
    }

    /// The folders of Section 10.2 that a walk reads now.
    #[must_use]
    pub fn found_roots(&self) -> Vec<Root> {
        found::roots(&self.tool_folders, &self.settings)
    }

    #[must_use]
    pub fn now(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    #[must_use]
    pub fn claim(&self) -> Claim {
        Claim {
            device: self.device,
            switch: if self.switch_on {
                Switch::On
            } else {
                Switch::Off
            },
            types: BUILT.to_vec(),
            kinds: vec![WorkKind::Model],
            os: self.os,
            architecture: self.architecture,
            // Model items do not use the job slot of K10 (Section 3).
            free: Free {
                slots: 0,
                memory_bytes: 0,
                disk_bytes: 0,
            },
            holds: Vec::new(),
            unclean: false,
        }
    }

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
                    arguments: command::llama_arguments(&settings, port),
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

    fn place(&self) -> Place<'_> {
        Place {
            folders: &self.folders,
            sources: &self.sources,
            os: self.os.as_str(),
            architecture: self.architecture.as_str(),
            reserve: self.reserve,
            program: &self.launcher,
        }
    }

    fn cache(&self) -> super::pull::Cache<'_> {
        super::pull::Cache {
            models: &self.folders.models,
            reserve: self.reserve,
            program: &self.launcher,
            settings: &self.settings,
        }
    }

    /// The result of `model_report` with the list `found` of a walk.
    async fn model_state(&self, found: Vec<Value>) -> Value {
        let servers = self.servers.lock().await;
        let mut model = state::model_state(&self.folders, &servers, &self.settings);
        model["found"] = Value::Array(found);
        model
    }

    /// # Errors
    /// The cause of Section 5 of the step.
    pub async fn run(&mut self, order: &Order) -> Result<Option<Value>, Cause> {
        if !BUILT.contains(&order.work_type) {
            return Err(Cause::TypeUnknown);
        }
        match &order.parameters {
            Parameters::Report => {
                let found = super::found::listed(self.found_roots()).await;
                Ok(Some(self.model_state(found).await))
            }
            Parameters::Adopt { copy, found } => {
                let state = self.folders.state.clone();
                super::found::adopt(self.found_roots(), state, *copy, found.clone())
                    .await
                    .map(Some)
            }
            Parameters::Install(release) => install::install(release, &self.place())
                .await
                .map(|()| None),
            Parameters::Start {
                server,
                release,
                settings,
            } => {
                super::folders::make_all(&self.folders).map_err(|_| Cause::LimitDisk)?;
                let started = self.server_for(*server, release, settings)?;
                let clock = self.started;
                self.servers.lock().await.stop(*server).await;
                let ready = Servers::ready(started, move || clock.elapsed().as_secs()).await?;
                self.servers.lock().await.map.insert(*server, ready);
                Ok(None)
            }
            // TODO(L1-10): count the requests in progress once the daemon sends any.
            Parameters::Drain { seconds } => {
                super::drain::drain(*seconds, || 0).await;
                Ok(None)
            }
            Parameters::Pull { .. } => super::pull::pull(&order.parameters, &self.cache()).await,
            Parameters::CopyDelete { copy } => {
                super::found::record::forget(&self.folders.state, *copy)?;
                super::pull::delete(&self.folders.models, *copy)
            }
            Parameters::Stop { server } => {
                if self.servers.lock().await.stop(*server).await {
                    Ok(None)
                } else {
                    Err(Cause::CopyUnknown)
                }
            }
            _ => Err(Cause::TypeUnknown),
        }
    }

    /// `Ok(None)` when the hub had no item.
    ///
    /// # Errors
    /// The cause of the hub when the claim or the report fails.
    pub async fn cycle(
        &mut self,
        client: &HubClient,
        token: &DaemonToken,
    ) -> Result<Option<Value>, HubCause> {
        let item = match client.claim_work(token, &self.claim()).await {
            Answer::Accepted(item) => item,
            Answer::Refused(code) => return Err(HubCause::Status(code.status())),
            Answer::Failed(cause) => return Err(cause),
        };
        let Some(item) = item else { return Ok(None) };
        // TODO(L1-5): take the memory share and GPUs from the device policy and inventory; until then none pass.
        let machine = ModelMachine {
            switch_on: self.switch_on,
            policy_permits: true,
            settings: &self.settings,
            limits: SettingLimits {
                memory_share: 1.0,
                gpus: &[],
            },
        };
        let start = Instant::now();
        let body = match order::check(&item, &machine) {
            Checked::Drop => return Ok(None),
            Checked::Refuse { id, attempt, cause } => report(id, attempt, Err(cause), 0),
            // TODO(L1-8, L1-16): the types that this build does not do yet.
            Checked::Start(order) if !BUILT.contains(&order.work_type) => {
                report(order.id, order.attempt, Err(Cause::TypeUnknown), 0)
            }
            Checked::Start(order) => {
                let running =
                    json!({ "id": order.id, "attempt": order.attempt, "state": "running" });
                if let Answer::Failed(cause) = client.report_work(token, &running).await {
                    return Err(cause);
                }
                let seconds = || start.elapsed().as_secs();
                match super::pull::watch::run(self, &order, client, token).await {
                    Watched::Ended(outcome) => report(order.id, order.attempt, outcome, seconds()),
                    Watched::Stopped => watch::stopped(order.id, order.attempt, seconds()),
                    Watched::Lost => return Ok(None),
                }
            }
        };
        match client.report_work(token, &body).await {
            Answer::Failed(cause) => Err(cause),
            Answer::Accepted(_) | Answer::Refused(_) => Ok(Some(body)),
        }
    }
}
