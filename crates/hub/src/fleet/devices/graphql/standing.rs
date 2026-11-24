//! What other domains add to `Device` and `Query.devices` (API-005, WEB-001, WEB-002, MET-004,
//! MET-006, MET-018, MET-032, R-122).

mod interfaces;
mod listing;
mod loaders;
mod metrics;
mod ranking;

pub use self::interfaces::{
    SuggestionsOf, WebInterfacesOf, web_interface_suggestions_loader, web_interfaces_loader,
};
pub use self::loaders::{ChecksOf, ResolvedIpOf, StatusOf, UptimeOf};
pub use self::metrics::{
    BatteriesOf, DaemonNeedOf, DaemonVersionOf, GpusOf, ReadingsOf, StaleOf, batteries_loader,
    daemon_need_loader, daemon_version_loader, gpus_loader, readings_loader, stale_loader,
};
pub(super) use self::{
    listing::list,
    ranking::{Position, decode as decode_cursor},
};
