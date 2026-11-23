//! `anima route list` (HOST-046).

use super::documents::{RouteList, route_list};
use super::shown;
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    /// Only the names of this application (its ID).
    #[arg(long)]
    application: Option<String>,
}

impl ListArgs {
    /// # Errors
    /// The hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let variables = route_list::Variables {
            application_id: self.application,
        };
        let routes = invocation
            .hub()?
            .run::<RouteList>(variables)
            .await?
            .application_routes;
        invocation
            .output
            .data(&routes, |routes| shown::table(routes))
    }
}
