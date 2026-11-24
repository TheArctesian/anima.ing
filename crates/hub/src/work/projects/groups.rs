//! Project groups: the operator's own labels, one for each project at most (PRO-059 to PRO-064).

pub(super) mod graphql;
mod imports;
mod rules;
pub(super) mod store;

pub(super) use self::graphql::GroupView;
