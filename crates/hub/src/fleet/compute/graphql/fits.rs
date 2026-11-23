//! The fit of each variant of a model on each machine that sent its hardware values (FIT-001,
//! FIT-005; task L1-20), with the memory share of its device policy (CAP-011).
#![expect(
    clippy::unused_async_trait_impl,
    reason = "the async-graphql Enum derive generates async trait impls"
)]

use async_graphql::{Enum, ID, InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use daemon_core::hardware::BackendName;
use sqlx::PgPool;

use crate::api_errors::FieldError;
use crate::fleet::compute::copies::graphql::{HostPlace, HostPlaceKind};
use crate::fleet::compute::library::{self, Model};
use crate::fleet::compute::placement::{
    Estimate, FitVerdict, Machine, OverheadTable, Placement, Unknown, estimate,
};
use crate::fleet::compute::policy;
use crate::fleet::{devices, metrics};
use crate::ids::DeviceId;

/// The machines and the context of a fit estimate.
#[derive(Clone, Debug, Default, InputObject)]
pub struct ModelFitFilter {
    /// Only this machine.
    pub device_id: Option<ID>,
    /// The tokens of context to estimate; the default is 8192, or the context of the model
    /// when it is smaller.
    pub context_tokens: Option<i32>,
}

/// Whether a variant fits a machine (FIT-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
pub enum ModelFitState {
    Fits,
    /// Only a smaller context fits (FIT-004).
    ReducedContext,
    TooLarge,
    /// The model files do not give the layer shape (FIT-002).
    Unknown,
}

/// Where the weights are: in the GPU, partly in the system memory, or on the CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
pub enum ModelPlacement {
    Gpu,
    Partial,
    Cpu,
}

/// The fit of one variant on one machine.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ModelFit")]
pub struct ModelFitView {
    /// The tag of the variant.
    pub variant: String,
    /// `name:tag`, as a pull takes it.
    pub address: String,
    pub place: HostPlace,
    pub state: ModelFitState,
    pub placement: Option<ModelPlacement>,
    /// `cuda`, `rocm`, `metal`, `vulkan` or `cpu`.
    pub backend: Option<String>,
    pub context_tokens: Option<i32>,
    pub largest_context: Option<i32>,
    pub size_bytes: f64,
    /// The weights, the KV cache and the overhead at the context of the estimate.
    pub needed_bytes: Option<f64>,
    /// The share of the memory that the device policy gives to models.
    pub usable_bytes: Option<f64>,
    pub reason: String,
}

/// What a fit query asks, after its checks.
#[derive(Clone, Debug)]
pub struct Asked {
    pub model: String,
    pub device: Option<DeviceId>,
    pub context: Option<u64>,
}

/// Why the fits cannot be given.
#[derive(Debug, thiserror::Error)]
pub enum FitsError {
    #[error(transparent)]
    Library(#[from] library::Error),
    #[error("{}", .0.message)]
    Validation(FieldError),
    #[error(transparent)]
    Devices(#[from] devices::Error),
    #[error(transparent)]
    Metrics(#[from] metrics::Error),
    #[error(transparent)]
    Policy(#[from] policy::Error),
}

impl Asked {
    /// The tag of a `name:tag`, which asks for one variant.
    fn tag(&self) -> Option<&str> {
        self.model.split_once(':').map(|(_, tag)| tag.trim())
    }

    /// # Errors
    /// `VALIDATION` on `deviceId` or `contextTokens`.
    pub fn of(model: String, filter: &ModelFitFilter) -> Result<Self, FieldError> {
        let device = filter
            .device_id
            .as_ref()
            .map(|id| id.parse::<DeviceId>())
            .transpose()
            .map_err(|_| FieldError::new("deviceId", "That id is not valid."))?;
        let context = filter
            .context_tokens
            .map(|tokens| u64::try_from(tokens).ok().filter(|tokens| *tokens > 0))
            .map(|tokens| {
                tokens.ok_or_else(|| {
                    FieldError::new("contextTokens", "Give a context of one token or more.")
                })
            })
            .transpose()?;
        Ok(Self {
            model,
            device,
            context,
        })
    }
}

/// The fit of each asked variant on each asked machine, in the order of the machine name and
/// the variants of the model.
///
/// # Errors
/// [`FitsError`] for an unknown model or tag, or a store that cannot be read.
pub async fn gather(
    db: &PgPool,
    asked: &Asked,
    now: DateTime<Utc>,
) -> Result<Vec<ModelFitView>, FitsError> {
    let model = library::get(db, &asked.model).await?;
    let tag = asked.tag();
    if tag.is_some_and(|tag| !model.variants.iter().any(|variant| variant.tag == tag)) {
        return Err(FitsError::Validation(FieldError::new(
            "model",
            "The model has no variant with that tag.",
        )));
    }
    let hardware = metrics::hardware(db).await?;
    let mut machines: Vec<_> = devices::list(db)
        .await?
        .into_iter()
        .filter(|device| asked.device.is_none_or(|id| id == device.id))
        .filter(|device| hardware.contains_key(&device.id))
        .collect();
    machines.sort_by(|one, other| one.name.cmp(&other.name));
    let ids: Vec<DeviceId> = machines.iter().map(|device| device.id).collect();
    let statuses = policy::statuses(db, &ids, now).await?;
    let overheads = OverheadTable::default();
    let mut fits = Vec::new();
    for (device, status) in machines.iter().zip(statuses) {
        let (values, _) = &hardware[&device.id];
        let machine = Machine {
            hardware: values,
            memory_share: status.policy.memory_share,
            overheads: &overheads,
        };
        let place = HostPlace {
            kind: HostPlaceKind::Machine,
            id: ID(device.id.to_string()),
            name: device.name.clone(),
        };
        fits.extend(of_model(&model, asked, machine, &place));
    }
    Ok(fits)
}

fn of_model(
    model: &Model,
    asked: &Asked,
    machine: Machine<'_>,
    place: &HostPlace,
) -> Vec<ModelFitView> {
    let tag = asked.tag();
    model
        .variants
        .iter()
        .filter(|variant| tag.is_none_or(|tag| variant.tag == tag))
        .map(|variant| {
            let found = estimate(&model.facts, variant, machine, asked.context);
            let mut view = view(found);
            view.variant.clone_from(&variant.tag);
            view.address = library::address(&model.name, &variant.tag);
            view.place = place.clone();
            view.size_bytes = float(variant.size_bytes());
            view
        })
        .collect()
}

/// The view of an estimate; the caller sets the variant, its address, the place and the size.
fn view(found: Result<Estimate, Unknown>) -> ModelFitView {
    let blank = ModelFitView {
        variant: String::new(),
        address: String::new(),
        place: HostPlace {
            kind: HostPlaceKind::Machine,
            id: ID(String::new()),
            name: String::new(),
        },
        state: ModelFitState::Unknown,
        placement: None,
        backend: None,
        context_tokens: None,
        largest_context: None,
        size_bytes: 0.0,
        needed_bytes: None,
        usable_bytes: None,
        reason: String::new(),
    };
    let estimate = match found {
        Ok(estimate) => estimate,
        Err(unknown) => {
            return ModelFitView {
                reason: unknown_reason(&unknown).to_owned(),
                ..blank
            };
        }
    };
    let request = estimate.request;
    let (state, needed, largest) = match estimate.verdict {
        FitVerdict::Fits { used_bytes } => (ModelFitState::Fits, used_bytes, None),
        FitVerdict::ReducedContext {
            largest_context,
            used_bytes,
        } => (
            ModelFitState::ReducedContext,
            used_bytes,
            Some(largest_context),
        ),
        FitVerdict::NoContext { missing_bytes } => (
            ModelFitState::TooLarge,
            request.usable_bytes.saturating_add(missing_bytes),
            None,
        ),
    };
    ModelFitView {
        state,
        placement: Some(placement(estimate.placement)),
        backend: Some(estimate.backend.as_str().to_owned()),
        context_tokens: Some(tokens(request.context_tokens)),
        largest_context: largest.map(tokens),
        needed_bytes: Some(float(needed)),
        usable_bytes: Some(float(request.usable_bytes)),
        reason: reason(&estimate),
        ..blank
    }
}

const fn placement(placement: Placement) -> ModelPlacement {
    match placement {
        Placement::Gpu => ModelPlacement::Gpu,
        Placement::Partial => ModelPlacement::Partial,
        Placement::Cpu => ModelPlacement::Cpu,
    }
}

const fn unknown_reason(unknown: &Unknown) -> &'static str {
    match unknown {
        Unknown::Shape => "The model files do not give the layer shape, so the fit is not known.",
        Unknown::Fit(_) => "A size of this variant is too large to estimate.",
    }
}

/// One sentence for a person: where the model runs, or what it lacks.
fn reason(estimate: &Estimate) -> String {
    let backend = backend_words(estimate.backend);
    let context = estimate.request.context_tokens;
    match (estimate.verdict, estimate.placement) {
        (FitVerdict::Fits { .. }, Placement::Gpu) => {
            format!("Fits in the memory of the GPU ({backend}) at {context} tokens of context.")
        }
        (FitVerdict::Fits { .. }, Placement::Partial) => {
            format!("Fits with part of the model in the system memory ({backend}); it runs slower.")
        }
        (FitVerdict::Fits { .. }, Placement::Cpu) => {
            "Fits in the system memory; this machine has no usable GPU, so it runs slower."
                .to_owned()
        }
        (
            FitVerdict::ReducedContext {
                largest_context, ..
            },
            _,
        ) => {
            format!("Fits only at {largest_context} tokens of context or fewer, not at {context}.")
        }
        (FitVerdict::NoContext { missing_bytes }, _) => format!(
            "Needs {} more than the {} that this machine gives to models.",
            gigabytes(missing_bytes),
            gigabytes(estimate.request.usable_bytes)
        ),
    }
}

const fn backend_words(backend: BackendName) -> &'static str {
    match backend {
        BackendName::Cuda => "CUDA",
        BackendName::Rocm => "ROCm",
        BackendName::Metal => "Metal",
        BackendName::Vulkan => "Vulkan",
        BackendName::Cpu | BackendName::Other => "CPU",
    }
}

fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", float(bytes) / 1e9)
}

fn tokens(count: u64) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// A count as a GraphQL `Float`: exact up to 2^53, which no model size reaches.
#[expect(clippy::cast_precision_loss, reason = "exact below 2^53")]
const fn float(value: u64) -> f64 {
    value as f64
}
