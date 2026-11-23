//! Which resources the signs of a project propose (PRO-003, PRO-004, PRO-006).

use std::collections::BTreeSet;

use super::model::{Held, ResourceKind, ResourceState, Sign};
use crate::ids::ResourceId;

/// No sign names a service that holds data about the operator (PRO-006).
#[must_use]
pub const fn kind_of(sign: Sign) -> ResourceKind {
    match sign {
        Sign::WranglerConfig => ResourceKind::Cloudflare,
        Sign::VercelJson | Sign::VercelDeployment => ResourceKind::Vercel,
        Sign::Dockerfile | Sign::ComposeFile => ResourceKind::Server,
        Sign::TauriConf => ResourceKind::DesktopApplication,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub add: Vec<ResourceKind>,
    pub drop: Vec<ResourceId>,
}

/// `signs` are of the project's repositories that are not left out.
#[must_use]
pub fn reconcile(signs: &[Sign], held: &[Held]) -> Plan {
    let signed: BTreeSet<ResourceKind> = signs.iter().copied().map(kind_of).collect();
    let add = signed
        .iter()
        .copied()
        .filter(|kind| !held.iter().any(|resource| resource.kind == *kind))
        .collect();
    let drop = held
        .iter()
        .filter(|resource| {
            resource.state == ResourceState::FoundInRepository && !signed.contains(&resource.kind)
        })
        .map(|resource| resource.id)
        .collect();
    Plan { add, drop }
}
