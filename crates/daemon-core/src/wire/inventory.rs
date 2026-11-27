//! The `inventory` member of a batch (K8 Section 4; CAP-020, CAP-024, CAP-029), and the hub's
//! pure check of it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::inventory::{
    Architecture, Inventory, MAX_BUILD_TARGETS, MAX_TOOLS, Os, Reason, Tool, ToolName, Verdict,
    is_build_target, is_tool_name, is_version,
};

/// It replaces the stored inventory whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchInventory {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mobile_present"
    )]
    pub mobile_tools: Option<crate::inventory::mobile::MobileTools>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_access: Option<crate::inventory::RemoteAccess>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_vnc_user: Option<String>,
    pub architecture: Architecture,
    pub os: Os,
    pub os_version: String,
    pub tools: Vec<BatchTool>,
    pub build_targets: Vec<String>,
}

/// A version, or a [`Reason`] as its `state` and no version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Reason>,
}

impl From<&Inventory> for BatchInventory {
    fn from(inventory: &Inventory) -> Self {
        Self {
            remote_access: inventory.remote_access.clone(),
            mobile_tools: inventory.mobile_tools.clone(),
            remote_vnc_user: inventory.remote_vnc_user.clone(),
            architecture: inventory.architecture,
            os: inventory.os,
            os_version: inventory.os_version.clone(),
            tools: inventory.tools.iter().map(BatchTool::from).collect(),
            build_targets: inventory.build_targets.clone(),
        }
    }
}

impl From<&Tool> for BatchTool {
    fn from(tool: &Tool) -> Self {
        Self {
            name: tool.name.as_str().to_owned(),
            version: tool.version().map(str::to_owned),
            state: tool.reason(),
        }
    }
}

impl BatchInventory {
    /// The inventory the hub stores, without the tools whose name it does not know; `None` when
    /// the member breaks K8 Section 4.
    #[must_use]
    pub fn accepted(self) -> Option<Inventory> {
        let names: BTreeSet<&str> = self.tools.iter().map(|tool| tool.name.as_str()).collect();
        let targets: BTreeSet<&str> = self.build_targets.iter().map(String::as_str).collect();
        let fits = self
            .remote_access
            .as_ref()
            .is_none_or(crate::inventory::RemoteAccess::accepted)
            && self
                .mobile_tools
                .as_ref()
                .is_none_or(crate::inventory::mobile::MobileTools::accepted)
            && self
                .remote_vnc_user
                .as_deref()
                .is_none_or(crate::inventory::is_remote_user)
            && is_version(&self.os_version)
            && self.tools.len() <= MAX_TOOLS
            && names.len() == self.tools.len()
            && self.tools.iter().all(BatchTool::fits)
            && self.build_targets.len() <= MAX_BUILD_TARGETS
            && targets.len() == self.build_targets.len()
            && targets.iter().all(|target| is_build_target(target));
        if !fits {
            return None;
        }
        let mut tools: Vec<Tool> = self
            .tools
            .into_iter()
            .filter_map(|tool| {
                let verdict = match (tool.version, tool.state) {
                    (Some(version), _) => Verdict::Usable(version),
                    (None, state) => Verdict::NotUsable(state.unwrap_or(Reason::Failed)),
                };
                Some(Tool {
                    name: ToolName::parse(&tool.name)?,
                    verdict,
                })
            })
            .collect();
        tools.sort_by_key(|tool| tool.name);
        let mut build_targets = self.build_targets;
        build_targets.sort_unstable();
        Some(Inventory {
            remote_access: self.remote_access,
            mobile_tools: self.mobile_tools,
            remote_vnc_user: self.remote_vnc_user,
            architecture: self.architecture,
            os: self.os,
            os_version: self.os_version,
            tools,
            build_targets,
        })
    }
}

impl BatchTool {
    fn fits(&self) -> bool {
        let told = match (&self.version, self.state) {
            (Some(version), None) => is_version(version),
            (None, Some(_)) => true,
            _ => false,
        };
        told && is_tool_name(&self.name)
    }
}

fn mobile_present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<crate::inventory::mobile::MobileTools>, D::Error> {
    crate::inventory::mobile::MobileTools::deserialize(deserializer).map(Some)
}
