//! K1 work operations (R-270). The project device allowlist is an operator trust decision.

pub(crate) mod documents;

use serde::{Deserialize, Serialize};

use crate::{failure::Failure, invocation::Invocation};

use documents::{
    JobDetail, JobDeviceExcluded, JobDeviceWork, JobList, JobProjectDevices, JobProjectMachine,
    JobStop, job_detail, job_device_excluded, job_device_work, job_list, job_project_devices,
    job_project_machine, job_stop,
};

#[derive(Clone, Copy, Debug, clap::ValueEnum, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum State {
    Queued,
    Claimed,
    Preparing,
    Running,
    Finishing,
    Done,
    Failed,
    Stopped,
}

impl From<State> for job_list::WorkJobState {
    fn from(state: State) -> Self {
        match state {
            State::Queued => Self::QUEUED,
            State::Claimed => Self::CLAIMED,
            State::Preparing => Self::PREPARING,
            State::Running => Self::RUNNING,
            State::Finishing => Self::FINISHING,
            State::Done => Self::DONE,
            State::Failed => Self::FAILED,
            State::Stopped => Self::STOPPED,
        }
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum JobCommand {
    /// List work and its placement reasons.
    List {
        #[arg(long, default_value_t = 50)]
        first: i64,
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        state: Option<State>,
    },
    /// Show a job and the bounded tail of its log.
    Get { id: String },
    /// Stop queued work or ask its holder to stop it.
    Stop { id: String },
    /// Set whether the hub gives work to one device.
    DeviceWork { device_id: String, on: bool },
    /// Replace one project's complete device allowlist for secret-bearing work.
    ProjectDevices {
        project_id: String,
        device_ids: Vec<String>,
    },
    /// Pin unnamed work to one device. Omit the device to remove the pin.
    ProjectMachine {
        project_id: String,
        device_id: Option<String>,
    },
    /// Set whether a device is excluded from unnamed work.
    DeviceExcluded { device_id: String, excluded: bool },
}

impl JobCommand {
    /// Runs one typed K1 operation.
    ///
    /// # Errors
    /// If sign-in, the hub request or output fails.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        match self {
            Self::List {
                first,
                after,
                state,
            } => {
                let vars = job_list::Variables {
                    first,
                    after,
                    state: state.map(Into::into),
                };
                print(invocation, &hub.run::<JobList>(vars).await?.jobs)
            }
            Self::Get { id } => print(
                invocation,
                &hub.run::<JobDetail>(job_detail::Variables { id })
                    .await?
                    .job,
            ),
            Self::Stop { id } => {
                let result = hub.run::<JobStop>(job_stop::Variables { id }).await?;
                print(invocation, &result)
            }
            Self::DeviceWork { device_id, on } => {
                let vars = job_device_work::Variables { device_id, on };
                let result = hub.run::<JobDeviceWork>(vars).await?;
                print(invocation, &result)
            }
            Self::ProjectDevices {
                project_id,
                device_ids,
            } => {
                let vars = job_project_devices::Variables {
                    project_id,
                    device_ids,
                };
                print(invocation, &hub.run::<JobProjectDevices>(vars).await?)
            }
            Self::ProjectMachine {
                project_id,
                device_id,
            } => {
                let vars = job_project_machine::Variables {
                    project_id,
                    device_id,
                };
                print(invocation, &hub.run::<JobProjectMachine>(vars).await?)
            }
            Self::DeviceExcluded {
                device_id,
                excluded,
            } => {
                let vars = job_device_excluded::Variables {
                    device_id,
                    excluded,
                };
                print(invocation, &hub.run::<JobDeviceExcluded>(vars).await?)
            }
        }
    }
}

fn print(invocation: &Invocation, value: &impl Serialize) -> Result<(), Failure> {
    let value = serde_json::to_value(value).map_err(|_| Failure::Write {
        reason: "Could not encode the work result.".to_owned(),
    })?;
    invocation.output.raw(&value)
}
