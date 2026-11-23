//! Issues and comments read live from GitHub (PRJ-031), and changes to them (OD-E14, PRJ-028).
//! GitHub lists pull requests as issues; the hub shows them only as pull requests.

mod back;
mod changes;
mod graphql;
#[cfg(test)]
mod tests;
mod views;

pub(super) use self::{changes::Mutation, graphql::Query};
