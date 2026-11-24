//! The preset and the answers of a `llama_server` router, as llama.cpp b12613 writes them (measured on pop-os,
//! 2026-11-23).

use std::path::Path;

use serde_json::json;
use uuid::Uuid;

use super::{Entry, Status, Use, model_files, preset, statuses, usage};
use crate::model_work::settings::{GpuLayers, ServerSettings};

fn copy(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

#[test]
fn the_preset_names_each_copy_once_in_a_fixed_order_with_the_settings_of_its_load() {
    let settings = ServerSettings {
        context_tokens: Some(8192),
        gpu_layers: Some(GpuLayers::All),
        ..ServerSettings::default()
    };
    let plain = ServerSettings::default();
    let second = Entry {
        copy: copy(2),
        model: Path::new("/m/copies/b/q.gguf"),
        projector: Some(Path::new("/m/copies/b/mmproj-F16.gguf")),
        settings: &plain,
    };
    let first = Entry {
        copy: copy(1),
        model: Path::new("/m/copies/a/q.gguf"),
        projector: None,
        settings: &settings,
    };
    let text = preset(&[second.clone(), first.clone()]).unwrap_or_default();
    assert_eq!(
        text,
        format!(
            "version = 1\n\n[{}]\nmodel = /m/copies/a/q.gguf\nctx-size = 8192\nn-gpu-layers = 999\n\n[{}]\nmodel = /m/copies/b/q.gguf\nmmproj = /m/copies/b/mmproj-F16.gguf\n",
            copy(1),
            copy(2)
        )
    );
    assert_eq!(preset(&[first, second]).unwrap_or_default(), text);
}

#[test]
fn a_path_that_could_end_a_line_or_start_a_section_is_refused() {
    let plain = ServerSettings::default();
    for path in [
        "relative.gguf",
        "/m/a\nmodel = /x.gguf",
        "/m/[x].gguf",
        "/m/a;b.gguf",
    ] {
        let entry = Entry {
            copy: copy(1),
            model: Path::new(path),
            projector: None,
            settings: &plain,
        };
        assert_eq!(preset(&[entry]), None, "{path:?}");
    }
}

#[test]
fn the_model_file_is_the_one_gguf_or_the_first_part_of_a_split_model() {
    let names = |list: &[&str]| {
        list.iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    };
    let one = names(&["Qwen3-8B-Q4_K_M.gguf", "README.md"]);
    assert_eq!(model_files(&one), Some(("Qwen3-8B-Q4_K_M.gguf", None)));
    let split = names(&[
        "m-00002-of-00002.gguf",
        "m-00001-of-00002.gguf",
        "mmproj-F16.gguf",
    ]);
    assert_eq!(
        model_files(&split),
        Some(("m-00001-of-00002.gguf", Some("mmproj-F16.gguf")))
    );
    assert_eq!(model_files(&names(&["a.gguf", "b.gguf"])), None);
    assert_eq!(model_files(&names(&["mmproj-F16.gguf"])), None);
}

#[test]
fn only_the_copies_of_the_preset_are_read_and_a_failed_load_is_its_own_state() {
    let answer = json!({ "data": [
        { "id": copy(1).to_string(), "source": "preset", "status": { "value": "loading" } },
        { "id": copy(2).to_string(), "source": "preset", "status": { "value": "loaded" } },
        { "id": copy(3).to_string(), "source": "preset", "status": { "value": "unloaded", "failed": true, "exit_code": 1 } },
        { "id": copy(4).to_string(), "source": "preset", "status": { "value": "unloaded" } },
        { "id": "unsloth/Qwen3-8B-GGUF:Q4_K_M", "source": "cache", "status": { "value": "loaded" } },
        { "id": copy(5).to_string(), "source": "models_dir", "status": { "value": "loaded" } },
    ]});
    assert_eq!(
        statuses(&answer),
        [
            (copy(1), Status::Loading),
            (copy(2), Status::Loaded),
            (copy(3), Status::Failed),
            (copy(4), Status::Unloaded),
        ]
    );
}

#[test]
fn the_use_of_a_copy_reads_from_its_counters() {
    let text = "# HELP llamacpp:n_decode_total Total number of llama_decode() calls\n\
                llamacpp:prompt_tokens_total 4\nllamacpp:n_decode_total 7\n\
                llamacpp:requests_processing 1\nllamacpp:requests_deferred 2\n";
    assert_eq!(
        usage(text),
        Some(Use {
            in_progress: 3,
            decodes: 7
        })
    );
    assert_eq!(usage("llamacpp:n_decode_total 7\n"), None);
    assert_eq!(
        usage("llamacpp:requests_processing -1\nllamacpp:n_decode_total 7\n"),
        None
    );
}
