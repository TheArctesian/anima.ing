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
pub mod model;
mod os_update;
mod read;
/// Its own tool set, kept with the other application tools of a device (H21).
pub use applications::routes;

use applications::catalogue_entries;

use rmcp::model::JsonObject;
use serde_json::Value;

use crate::hub::Hub;
use crate::mcp::definition::Definition;
use crate::mcp::outcome::ToolError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceTool {
    OsUpdate,
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
    pub const ALL: [Self; 25] = [
        Self::OsUpdate,
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
            Self::OsUpdate => os_update::definition(),
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

    /// # Errors
    /// [`ToolError`] for wrong arguments or the hub's refusal.
    pub async fn call(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        match self {
            Self::OsUpdate => os_update::call(hub, arguments).await,
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

    async fn agents(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        if matches!(self, Self::AgentSessions) {
            agent_sessions::sessions(hub, arguments).await
        } else {
            agent_usage::usage(hub, arguments).await
        }
    }
}
