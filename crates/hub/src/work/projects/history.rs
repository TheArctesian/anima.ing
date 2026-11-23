//! History read live from GitHub (PRJ-030, PRJ-031) and commit statuses (OD-E14, PRO-010).

mod changes;
mod graphql;
mod omitted;
mod views;

pub(super) use self::{changes::Mutation, graphql::Query};
