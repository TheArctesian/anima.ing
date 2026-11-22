//! The `devices` tool set: the fleet, devices, their metrics and capabilities, the use of their
//! AI agent programs and their single sessions, their containers, the text of a Compose file, the
//! catalogue of applications, sites, wake, what a daemon waits for, and the changes.

mod agent_sessions;
mod agent_usage;
mod applications;
mod capabilities;
mod change;
mod compose_file;
mod compute_policy;
mod containers;
mod daemon_need;
mod metrics;
pub mod models;
mod read;
/// The public names of applications (task H21): its own tool set, kept with the other
/// application tools of a device.
pub use applications::routes;

use applications::catalogue_entries;

use rmcp::model::JsonObject;
use serde_json::Value;

use crate::hub::Hub;
use crate::mcp::definition::Definition;
use crate::mcp::outcome::ToolError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceTool {
    FleetOverview,
    Search,
    Get,
    Metrics,
    CapabilityList,
    ComputePolicyList,
    ComputePolicySet,
    AgentUsage,
    AgentSessions,
    ContainerList,
    ContainerStorage,
    ContainerNetworks,
    ContainerUpdates,
    ComposeCheck,
    ComposeFromRun,
    CatalogueList,
    CatalogueShow,
    CatalogueInstall,
    Create,
    SiteCreate,
    Wake,
    Update,
    Delete,
    DaemonNeedSet,
}

impl DeviceTool {
    pub const ALL: [Self; 24] = [
        Self::FleetOverview,
        Self::Search,
        Self::Get,
        Self::Metrics,
        Self::CapabilityList,
        Self::ComputePolicyList,
        Self::AgentUsage,
        Self::AgentSessions,
        Self::ContainerList,
        Self::ContainerStorage,
        Self::ContainerNetworks,
        Self::ContainerUpdates,
        Self::ComposeCheck,
        Self::ComposeFromRun,
        Self::CatalogueList,
        Self::CatalogueShow,
        Self::CatalogueInstall,
        Self::Create,
        Self::SiteCreate,
        Self::Wake,
        Self::Update,
        Self::Delete,
        Self::ComputePolicySet,
        Self::DaemonNeedSet,
    ];

    #[must_use]
    pub fn definition(self) -> Definition {
        match self {
            Self::Metrics => metrics::definition(),
            Self::CapabilityList => capabilities::definition(),
            Self::ComputePolicyList | Self::ComputePolicySet => compute_policy::definition_of(self),
            Self::DaemonNeedSet => daemon_need::definition(),
            Self::AgentUsage => agent_usage::definition(),
            Self::AgentSessions => agent_sessions::definition(),
            Self::ContainerList
            | Self::ContainerStorage
            | Self::ContainerNetworks
            | Self::ContainerUpdates => containers::definition_of(self),
            Self::ComposeCheck | Self::ComposeFromRun => compose_file::definition_of(self),
            Self::CatalogueList | Self::CatalogueShow | Self::CatalogueInstall => {
                catalogue_entries::definition_of(self)
            }
            Self::FleetOverview | Self::Search | Self::Get => read::definition(self),
            other => change::definition(other),
        }
    }

    /// Runs the tool.
    ///
    /// # Errors
    /// [`ToolError`] for wrong arguments or the hub's refusal.
    pub async fn call(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        match self {
            Self::FleetOverview => read::overview(hub).await,
            Self::Search => read::search(hub, arguments).await,
            Self::Get => read::get(hub, arguments).await,
            Self::Metrics => metrics::metrics(hub, arguments).await,
            Self::CapabilityList => capabilities::list(hub, arguments).await,
            Self::ComputePolicyList | Self::ComputePolicySet => {
                compute_policy::call(self, hub, arguments).await
            }
            Self::DaemonNeedSet => daemon_need::set(hub, arguments).await,
            Self::AgentUsage | Self::AgentSessions => self.agents(hub, arguments).await,
            Self::ContainerList
            | Self::ContainerStorage
            | Self::ContainerNetworks
            | Self::ContainerUpdates => containers::call(self, hub, arguments).await,
            Self::ComposeCheck | Self::ComposeFromRun => {
                compose_file::call(self, hub, arguments).await
            }
            Self::CatalogueList | Self::CatalogueShow | Self::CatalogueInstall => {
                catalogue_entries::call(self, hub, arguments).await
            }
            other => change::call(other, hub, arguments).await,
        }
    }

    /// Runs a tool of the use of the AI agent programs: the sums, or the single sessions.
    async fn agents(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        if matches!(self, Self::AgentSessions) {
            agent_sessions::sessions(hub, arguments).await
        } else {
            agent_usage::usage(hub, arguments).await
        }
    }
}
