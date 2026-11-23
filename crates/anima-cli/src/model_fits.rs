//! `anima model fit <model>`: whether each variant of a model fits each machine, on its GPU,
//! partly in its system memory or on its CPU, at a context (FIT-001 to FIT-005; task L1-20).

use graphql_client::GraphQLQuery;

use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::hub::Hub;
use crate::invocation::Invocation;
use crate::model_library::gib;
use crate::output::Table;
use crate::output::text::enum_name;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_fits/fit.graphql",
    response_derives = "Debug, Serialize, Clone",
    variables_derives = "Debug"
)]
pub struct ModelFits;

pub type Fit = model_fits::ModelFitsModelFits;

#[derive(Debug, clap::Args)]
pub struct FitArgs {
    /// The model's name, an alias, or name:tag for one variant.
    model: String,
    /// Only this device: its ID or its exact name.
    #[arg(long)]
    device: Option<String>,
    /// The tokens of context; the default is 8192, or the context of the model when it is
    /// smaller.
    #[arg(long)]
    context: Option<u32>,
}

/// What a fit asks: the model, and optionally one device and a context.
#[derive(Clone, Debug, Default)]
pub struct Asked {
    pub model: String,
    pub device: Option<String>,
    pub context: Option<u32>,
}

/// The fits that the hub gives for `asked`.
///
/// # Errors
/// [`Failure::NotFound`] for an unknown device or model; the hub's refusal.
pub async fn fetch(hub: &Hub, asked: Asked) -> Result<Vec<Fit>, Failure> {
    let device_id = match asked.device {
        Some(device) => Some(resolve(hub, &device).await?),
        None => None,
    };
    let filter = model_fits::ModelFitFilter {
        device_id,
        context_tokens: asked.context.map(i64::from),
    };
    let variables = model_fits::Variables {
        model: asked.model,
        filter: Some(filter),
    };
    Ok(hub.run::<ModelFits>(variables).await?.model_fits)
}

impl FitArgs {
    /// Prints one row for each variant on each machine, or with `--json` the K1 `ModelFit` list.
    ///
    /// # Errors
    /// As [`fetch`].
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let asked = Asked {
            model: self.model,
            device: self.device,
            context: self.context,
        };
        let fits = fetch(&invocation.hub()?, asked).await?;
        invocation.output.data(&fits, |fits| table(fits))?;
        if fits.is_empty() {
            invocation.output.note(
                "No machine sent its hardware values. Start anima-daemon on a machine first.",
            );
        }
        Ok(())
    }
}

fn table(fits: &[Fit]) -> Table {
    let mut table = Table::new(vec![
        "MACHINE", "VARIANT", "SIZE", "FIT", "RUNS ON", "CONTEXT", "REASON",
    ]);
    for fit in fits {
        let runs_on = match (&fit.placement, &fit.backend) {
            (Some(placement), Some(backend)) => format!("{} ({backend})", enum_name(placement)),
            _ => "-".to_owned(),
        };
        let context = fit
            .largest_context
            .or(fit.context_tokens)
            .map_or_else(|| "-".to_owned(), |tokens| tokens.to_string());
        table.row(vec![
            fit.place.name.as_str().into(),
            fit.variant.as_str().into(),
            gib(fit.size_bytes).into(),
            enum_name(&fit.state).into(),
            runs_on.into(),
            context.into(),
            fit.reason.as_str().into(),
        ]);
    }
    table
}
