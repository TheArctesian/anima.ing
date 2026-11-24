//! One cycle of model work (Section 3, CAG-002, CAG-003); the caller runs it only while the switch at the machine is on.

use std::path::PathBuf;
use std::time::Instant;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::found::{self, Root};
use daemon_core::model_work::machine::MachineSettings;
use daemon_core::model_work::order::{self, Checked, ModelMachine, Order, Parameters};
use daemon_core::model_work::releases::Sources;
use daemon_core::model_work::settings::SettingLimits;
use daemon_core::wire::{
    Claim, DaemonToken, Free, Switch, WorkArchitecture, WorkKind, WorkOs, WorkState, WorkType,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::install::{self, Place};
use super::pull::watch::Watched;
use super::reports::{self, ended, refused};
use super::state;
use super::supervise::Shared;
use crate::hub::HubClient;
use crate::hub::answer::{Answer, Cause as HubCause};

/// The folder of a server's temporary files, below its folder in the state folder.
pub const TEMPORARY: &str = "tmp";

/// The preset of a server in router mode, in its folder (K11 Section 11.1).
pub const PRESET: &str = "preset.ini";

mod start;

pub const BUILT: [WorkType; 11] = [
    WorkType::ModelReport,
    WorkType::ModelDrain,
    WorkType::RuntimeInstall,
    WorkType::ModelPull,
    WorkType::CopyDelete,
    WorkType::CopyAdopt,
    WorkType::ServerStart,
    WorkType::ServerStop,
    WorkType::ModelLoad,
    WorkType::ModelUnload,
    WorkType::ModelKeep,
];

pub struct Node {
    pub device: Uuid,
    pub os: WorkOs,
    pub architecture: WorkArchitecture,
    pub folders: Folders,
    pub sources: Sources,
    pub settings: MachineSettings,
    pub servers: Shared,
    /// The loads of the copies (Section 11.1).
    pub loads: std::sync::Arc<super::load::Shared>,
    pub switch_on: bool,
    pub started: Instant,
    pub reserve: u64,
    /// The daemon program: the launcher of a runtime on Linux (R-331).
    pub launcher: PathBuf,
    pub ports: std::ops::RangeInclusive<u16>,
    /// The fixed folders of Section 10.2 for this installation (`found::tool_folders`).
    pub tool_folders: Vec<Root>,
    /// How often a held item is reported while its step runs: `watch::EVERY`.
    pub report_every: std::time::Duration,
}

/// A report of a step of the attempt with no result (K10 Section 7).
async fn moved(
    client: &HubClient,
    token: &DaemonToken,
    order: &Order,
    state: WorkState,
) -> Result<(), HubCause> {
    let body = json!({ "id": order.id, "attempt": order.attempt, "state": state.as_str() });
    match client.report_work(token, &body).await {
        Answer::Failed(cause) => Err(cause),
        Answer::Accepted(_) | Answer::Refused(_) => Ok(()),
    }
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
        let loads = self.loads.report(&super::load::usable(1.0)).await;
        let servers = self.servers.lock().await;
        let mut model = state::model_state(&self.folders, &servers, &self.settings, loads);
        model["found"] = Value::Array(found);
        model
    }

    /// `copy_delete`: the copy is unloaded and its keep file, found record and files are deleted.
    async fn delete_copy(&self, copy: Uuid) -> Result<Option<Value>, Cause> {
        let routers = super::load::routers(&self.servers).await;
        let memory = super::load::usable(1.0);
        let at = super::load::At {
            routers: &routers,
            usable: &memory,
            now: super::load::now(),
        };
        self.loads.for_order().await.forget(&at, copy).await?;
        super::found::record::forget(&self.folders.state, copy)?;
        super::pull::delete(&self.folders.models, copy)
    }

    /// # Errors
    /// The cause of Section 5 of the step.
    pub async fn run(&mut self, order: &Order) -> Result<Option<Value>, Cause> {
        if !BUILT.contains(&order.work_type) {
            return Err(Cause::TypeUnknown);
        }
        if let Some(done) = self.loads.run(&order.parameters, &self.servers).await {
            return done.map(|()| None);
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
            } => self.start(*server, release, settings).await.map(|()| None),
            // TODO(L1-10): count the requests in progress once the daemon sends any.
            Parameters::Drain { seconds } => {
                super::drain::drain(*seconds, || 0).await;
                Ok(None)
            }
            Parameters::Pull { .. } => super::pull::pull(&order.parameters, &self.cache()).await,
            Parameters::CopyDelete { copy } => self.delete_copy(*copy).await,
            Parameters::Stop { server } => {
                self.loads.for_order().await.server_stopped(*server);
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
            Checked::Refuse { id, attempt, cause } => refused(id, attempt, cause),
            // TODO(L1-8): `model_measure`, which this build does not do yet.
            Checked::Start(order) if !BUILT.contains(&order.work_type) => {
                refused(order.id, order.attempt, Cause::TypeUnknown)
            }
            Checked::Start(order) => {
                // K11 Section 3: `preparing` is the check of the item, `running` the step and
                // `finishing` its clean-up, in the diagram of K10 Section 7.
                moved(client, token, &order, WorkState::Preparing).await?;
                moved(client, token, &order, WorkState::Running).await?;
                let seconds = || start.elapsed().as_secs();
                match super::pull::watch::run(self, &order, client, token).await {
                    Watched::Ended(outcome) => {
                        moved(client, token, &order, WorkState::Finishing).await?;
                        ended(order.id, order.attempt, outcome, seconds())
                    }
                    Watched::Stopped => reports::stopped(order.id, order.attempt, seconds()),
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
