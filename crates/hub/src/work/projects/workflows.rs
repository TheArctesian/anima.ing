//! The workflows of a repository and their runs (PRJ-031, PRJ-034, PRJ-035, PRJ-028, OD-E14).

mod changes;
mod controls;
mod dispatch;
mod graphql;
pub(super) mod jobs;
#[cfg(test)]
mod tests;
mod views;

pub(super) use self::{changes::Mutation, graphql::Query};
