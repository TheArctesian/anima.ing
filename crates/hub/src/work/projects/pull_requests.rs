//! Pull requests, read live from GitHub and merged by the merge rule
//! (PRJ-031, PRJ-033, OD-E14, PRO-013).

mod back;
mod bulk;
mod changes;
mod graphql;
mod input;
mod merge;
#[cfg(test)]
mod tests;
mod views;

pub(super) use self::{changes::Mutation, graphql::Query};
