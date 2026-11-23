//! The fit of a variant on a machine from the hardware values (task L1-20; FIT-005).

use daemon_core::hardware::{
    Backend, BackendName, BackendReason, Hardware, UsableMemory, UsableSource,
};

use super::{FitVerdict, Machine, OverheadTable, Placement, Unknown, estimate};
use crate::fleet::compute::library::{Facts, Format, LayerShape, Variant, VariantFile};

const GIB: u64 = 1024 * 1024 * 1024;

// Qwen/Qwen2.5-32B-Instruct: 64 layers, hidden size 5120, 40 attention heads, 8 KV heads.
fn qwen_32b() -> Facts {
    Facts {
        context_length: Some(32_768),
        shape: LayerShape {
            layers: Some(64),
            hidden_size: Some(5_120),
            attention_heads: Some(40),
            key_value_heads: Some(8),
        },
        ..Facts::default()
    }
}

fn variant(size_bytes: u64) -> Variant {
    Variant {
        tag: "32b-q4_k_m".to_owned(),
        format: Format::Gguf,
        quantisation: "Q4_K_M".to_owned(),
        repository: "Qwen/Qwen2.5-32B-Instruct-GGUF".to_owned(),
        revision: "a".repeat(40),
        files: vec![VariantFile {
            path: "model.gguf".to_owned(),
            size_bytes,
            sha256: "b".repeat(64),
        }],
    }
}

fn backend(name: BackendName, state: Option<BackendReason>) -> Backend {
    Backend { name, state }
}

fn machine(system: u64, video: Option<u64>, backends: Vec<Backend>) -> Hardware {
    Hardware {
        system_memory_bytes: system,
        unified_memory_bytes: None,
        dedicated_memory_bytes: video,
        usable_memory: None,
        free_disk_bytes: None,
        cpu_features: Vec::new(),
        backends,
        power_source: None,
        thermal_state: None,
        operator_active: None,
    }
}

fn on(hardware: &Hardware, share: u8, size: u64, context: Option<u64>) -> super::Estimate {
    let overheads = OverheadTable::default();
    let machine = Machine {
        hardware,
        memory_share: share,
        overheads: &overheads,
    };
    estimate(&qwen_32b(), &variant(size), machine, context).expect("an estimate")
}

#[test]
fn fit_005_a_model_that_fits_the_video_memory_runs_on_the_gpu_at_the_default_context() {
    let pop = machine(
        94 * GIB,
        Some(24 * GIB),
        vec![
            backend(BackendName::Cuda, None),
            backend(BackendName::Cpu, None),
        ],
    );
    let found = on(&pop, 100, 19_850_000_000, None);
    assert_eq!(
        (found.backend, found.placement),
        (BackendName::Cuda, Placement::Gpu)
    );
    assert_eq!(found.request.context_tokens, 8_192);
    assert!(
        matches!(found.verdict, FitVerdict::Fits { .. }),
        "{:?}",
        found.verdict
    );
}

#[test]
fn fit_005_a_model_larger_than_the_video_memory_is_partial_with_the_system_memory() {
    let pop = machine(
        94 * GIB,
        Some(24 * GIB),
        vec![backend(BackendName::Cuda, None)],
    );
    let found = on(&pop, 100, 42_520_000_000, None);
    assert_eq!(found.placement, Placement::Partial);
    assert_eq!(found.request.usable_bytes, 118 * GIB);
    assert!(matches!(found.verdict, FitVerdict::Fits { .. }));
}

#[test]
fn fit_005_a_gpu_that_is_not_usable_leaves_the_system_memory_and_the_share_counts() {
    let box_ = machine(
        16 * GIB,
        Some(24 * GIB),
        vec![
            backend(BackendName::Cuda, Some(BackendReason::NoDriver)),
            backend(BackendName::Cpu, None),
        ],
    );
    let found = on(&box_, 50, 19_850_000_000, None);
    assert_eq!(
        (found.backend, found.placement),
        (BackendName::Cpu, Placement::Cpu)
    );
    assert_eq!(found.request.usable_bytes, 8 * GIB);
    assert!(matches!(found.verdict, FitVerdict::NoContext { .. }));
}

#[test]
fn fit_004_apple_silicon_uses_its_usable_memory_and_gives_the_largest_context() {
    let mut mac = machine(24 * GIB, None, vec![backend(BackendName::Metal, None)]);
    mac.unified_memory_bytes = Some(24 * GIB);
    mac.usable_memory = Some(UsableMemory {
        bytes: 21 * GIB,
        source: UsableSource::WiredLimit,
    });
    let found = on(&mac, 100, 19_850_000_000, Some(32_768));
    assert_eq!(
        (found.backend, found.placement),
        (BackendName::Metal, Placement::Gpu)
    );
    assert_eq!(found.request.usable_bytes, 21 * GIB);
    let FitVerdict::ReducedContext {
        largest_context, ..
    } = found.verdict
    else {
        panic!("{:?}", found.verdict);
    };
    assert!(largest_context < 32_768 && largest_context % 256 == 0);
}

#[test]
fn fit_002_a_model_with_no_layer_shape_has_no_estimate() {
    let pop = machine(94 * GIB, None, vec![backend(BackendName::Cpu, None)]);
    let overheads = OverheadTable::default();
    let machine = Machine {
        hardware: &pop,
        memory_share: 100,
        overheads: &overheads,
    };
    assert_eq!(
        estimate(&Facts::default(), &variant(1), machine, None),
        Err(Unknown::Shape)
    );
}
