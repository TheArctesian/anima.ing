//! Which series a value belongs to (MET-014, K8 Section 4).

use super::{Kind, Sample};

/// The highest GPU index a series or a GPU may carry (K8 Section 4).
pub const MAX_GPU_INDEX: u8 = 63;

/// The most series one point or one batch holds (K8 Section 4).
pub const MAX_SERIES: usize = 256;

const LABEL_BYTES: usize = 128;

/// Built only by [`SeriesKey::new`], so every key is one K8 Section 4 allows. The order lets a
/// batch list its series the same way each time.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SeriesKey {
    kind: Kind,
    label: Option<String>,
    gpu: Option<u8>,
}

impl SeriesKey {
    /// `None` when K8 Section 4 does not allow it: a label only for a labelled kind, a GPU index
    /// only for a GPU kind.
    #[must_use]
    pub fn new(kind: Kind, label: Option<String>, gpu: Option<u8>) -> Option<Self> {
        let label_fits = match &label {
            Some(text) => kind.is_labelled() && is_label(text),
            None => !kind.is_labelled(),
        };
        let gpu_fits = match gpu {
            Some(index) => kind.is_per_gpu() && index <= MAX_GPU_INDEX,
            None => !kind.is_per_gpu(),
        };
        (label_fits && gpu_fits).then_some(Self { kind, label, gpu })
    }

    /// The key of a collection sample; a GPU kind's label, the same as its index, stays out of
    /// the key (R-099).
    #[must_use]
    pub fn of(sample: &Sample) -> Option<Self> {
        let label = sample.kind.is_labelled().then(|| sample.label.clone());
        Self::new(sample.kind, label, sample.gpu_index)
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The mount point, interface or drive name; `None` for the kinds without one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[must_use]
    pub const fn gpu(&self) -> Option<u8> {
        self.gpu
    }
}

/// Whether `text` can be a series label or a GPU name (K8 Section 4).
pub(crate) fn is_label(text: &str) -> bool {
    (1..=LABEL_BYTES).contains(&text.len()) && !text.chars().any(char::is_control)
}
