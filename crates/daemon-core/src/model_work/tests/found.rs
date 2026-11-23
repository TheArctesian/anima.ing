//! Found models (Section 2.3, 8, 10.1 and 10.2; PULL-012, R-443, R-444).

use serde_json::{Value, json};
use uuid::Uuid;

use crate::model_work::cause::Cause;
use crate::model_work::found::huggingface::{blob_of_link, repository};
use crate::model_work::found::ollama::{layers, shown_name};
use crate::model_work::found::record::{Observed, Record, RecordFile, named_hash_holds};
use crate::model_work::found::{
    FoundFile, FoundModel, Tool, gguf_models, is_shown_name, key, owner_permits,
};
use crate::model_work::machine::{MachineSettings, SettingsError};
use crate::model_work::order::{Checked, ModelMachine, Parameters, check};
use crate::model_work::settings::SettingLimits;

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn adopt_item(parameters: &Value) -> Value {
    json!({
        "id": "0192f6aa-0000-7000-8000-000000000001", "type": "copy_adopt", "kind": "model",
        "attempt": 1, "lease": { "until": "2026-11-20T10:01:30Z" },
        "limits": { "seconds": 600, "idleSeconds": 60, "memoryBytes": 1, "diskBytes": 1, "orphanSeconds": 60 },
        "project": null, "secrets": [], "parameters": parameters, "actor": "User",
    })
}

fn checked(answer: &Value) -> Checked {
    let settings = MachineSettings::default();
    let machine = ModelMachine {
        switch_on: true,
        policy_permits: true,
        settings: &settings,
        limits: SettingLimits {
            memory_share: 0.5,
            gpus: &[0],
        },
    };
    check(answer, &machine)
}

#[test]
fn pull_012_copy_adopt_takes_a_key_and_never_a_path() {
    let copy = Uuid::now_v7();
    let found = key(Tool::Ollama, "registry.ollama.ai/library/qwen3/8b");

    let Checked::Start(order) = checked(&adopt_item(&json!({ "copy": copy, "found": found })))
    else {
        panic!("a key is taken");
    };
    assert_eq!(order.parameters, Parameters::Adopt { copy, found });

    for bad in [
        json!({ "copy": copy, "found": "/home/me/.ollama/models/blobs/x" }),
        json!({ "copy": copy, "found": SHA_A.to_uppercase() }),
        json!({ "copy": copy, "found": SHA_A, "path": "/tmp" }),
    ] {
        let Checked::Refuse { cause, .. } = checked(&adopt_item(&bad)) else {
            panic!("{bad} is refused");
        };
        assert_eq!(cause, Cause::MemberUnknown, "{bad}");
    }
}

#[test]
fn pull_012_a_key_is_stable_and_names_the_tool_and_the_place() {
    let place = "registry.ollama.ai/library/qwen3/8b";

    assert_eq!(key(Tool::Ollama, place), key(Tool::Ollama, place));
    assert_ne!(key(Tool::Ollama, place), key(Tool::Folder, place));
    assert_ne!(
        key(Tool::Ollama, place),
        key(Tool::Ollama, "registry.ollama.ai/library/qwen3/4b")
    );
    assert_eq!(key(Tool::Ollama, place).len(), 64);
}

#[test]
fn pull_012_split_gguf_parts_are_one_model_and_a_missing_part_is_none() {
    let names = [
        "b-00002-of-00002.gguf",
        "b-00001-of-00002.gguf",
        "a.gguf",
        "c-00001-of-00003.gguf",
        "c-00003-of-00003.gguf",
        "README.md",
        "x.py",
    ];

    assert_eq!(
        gguf_models(&names),
        vec![
            vec!["a.gguf"],
            vec!["b-00001-of-00002.gguf", "b-00002-of-00002.gguf"]
        ]
    );
}

#[test]
fn pull_012_an_ollama_manifest_gives_its_model_and_projector_layers() {
    let manifest = json!({ "schemaVersion": 2, "layers": [
        { "mediaType": "application/vnd.ollama.image.model", "digest": format!("sha256:{SHA_A}"), "size": 5 },
        { "mediaType": "application/vnd.ollama.image.projector", "digest": format!("sha256:{SHA_B}"), "size": 3 },
        { "mediaType": "application/vnd.ollama.image.template", "digest": "sha256:x", "size": 1 },
    ]});

    let found = layers(manifest.to_string().as_bytes()).expect("layers");

    assert_eq!(found.model.sha256, SHA_A);
    assert_eq!(found.model.file_name(), format!("sha256-{SHA_A}"));
    assert_eq!(found.projector.map(|blob| blob.size_bytes), Some(3));
}

#[test]
fn pull_012_an_ollama_manifest_of_another_form_is_not_a_model() {
    let model = |digest: &str| json!({ "mediaType": "application/vnd.ollama.image.model", "digest": digest, "size": 5 });
    let sha = format!("sha256:{SHA_A}");
    for layers_of in [
        json!([]),
        json!([model(&sha), model(&sha)]),
        json!([model("sha256:../../etc/passwd")]),
        json!([model(&format!("md5:{SHA_A}"))]),
    ] {
        let manifest = json!({ "layers": layers_of }).to_string();
        assert_eq!(layers(manifest.as_bytes()), None, "{manifest}");
    }
    assert_eq!(layers(&vec![b' '; 64 * 1024 + 1]), None, "too large");
}

#[test]
fn pull_012_ollama_names_are_shown_as_ollama_shows_them() {
    assert_eq!(
        shown_name(["registry.ollama.ai", "library", "qwen3", "8b"]),
        "qwen3:8b"
    );
    assert_eq!(
        shown_name(["registry.ollama.ai", "me", "tiny", "latest"]),
        "me/tiny:latest"
    );
    assert_eq!(
        shown_name(["hf.co", "unsloth", "Qwen3-8B-GGUF", "Q4_K_M"]),
        "hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M"
    );
    assert!(is_shown_name("hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M"));
    assert!(!is_shown_name("a b"));
    assert!(!is_shown_name("x\u{1b}[31m"));
}

#[test]
fn pull_012_the_hugging_face_cache_follows_only_a_link_to_its_own_blobs() {
    assert_eq!(
        repository("models--unsloth--Qwen3-8B-GGUF"),
        Some(("unsloth", "Qwen3-8B-GGUF"))
    );
    assert_eq!(repository("datasets--a--b"), None);
    assert_eq!(repository("models--..--b"), None);

    assert_eq!(
        blob_of_link(format!("../../blobs/{SHA_A}").as_bytes()),
        Some(SHA_A)
    );
    for text in [
        "/etc/passwd".to_owned(),
        "../../../etc/passwd".to_owned(),
        format!("../../blobs/../{SHA_A}"),
        format!("../blobs/{SHA_A}"),
        "../../blobs/x".to_owned(),
    ] {
        assert_eq!(blob_of_link(text.as_bytes()), None, "{text}");
    }
}

#[test]
fn pull_012_an_entry_that_another_user_can_write_is_not_read() {
    let (daemon, ollama) = (1000, 996);

    assert!(owner_permits(daemon, false, daemon, daemon));
    assert!(owner_permits(ollama, false, daemon, ollama));
    assert!(owner_permits(0, false, daemon, daemon));
    assert!(!owner_permits(1001, false, daemon, daemon));
    assert!(!owner_permits(daemon, true, daemon, daemon));
}

fn record() -> Record {
    Record {
        copy: Uuid::now_v7(),
        found: SHA_B.to_owned(),
        tool: Tool::Ollama,
        root: "/home/me/.ollama/models".to_owned(),
        files: vec![RecordFile {
            location: vec!["blobs".to_owned(), format!("sha256-{SHA_A}")],
            size_bytes: 5,
            sha256: SHA_A.to_owned(),
            device: 1,
            inode: 2,
            modified: 3,
        }],
    }
}

#[test]
fn pull_012_a_file_that_changed_since_it_was_adopted_is_refused_at_the_load() {
    let file = &record().files[0];

    assert_eq!(
        file.holds(Observed {
            size_bytes: 5,
            sha256: SHA_A
        }),
        Ok(())
    );
    assert_eq!(
        file.holds(Observed {
            size_bytes: 6,
            sha256: SHA_A
        }),
        Err(Cause::CopyChanged)
    );
    assert_eq!(
        file.holds(Observed {
            size_bytes: 5,
            sha256: SHA_B
        }),
        Err(Cause::CopyChanged)
    );
    assert!(!Cause::CopyChanged.is_refusal());
    assert!(!Cause::CopyMissing.is_refusal());
    assert!(Cause::FoundUnknown.is_refusal());
    assert!(Cause::is_code("copy_changed") && Cause::is_code("found_unknown"));
}

#[test]
fn pull_012_a_record_that_was_changed_on_disk_does_not_hold_its_form() {
    assert!(record().holds_form());
    let mut climbs = record();
    climbs.files[0].location = vec!["..".to_owned(), "x".to_owned()];
    let mut relative = record();
    relative.root = "models".to_owned();
    let mut empty = record();
    empty.files.clear();

    for bad in [climbs, relative, empty] {
        assert!(!bad.holds_form(), "{bad:?}");
    }
}

#[test]
fn pull_012_the_adopt_result_has_the_hashes_and_no_name() {
    let result = record().result();

    assert_eq!(result["origin"], "found");
    assert_eq!(result["copyState"], "downloaded");
    assert_eq!(
        result["fileHashes"],
        json!([{ "sizeBytes": 5, "sha256": SHA_A }])
    );
    assert!(!result.to_string().contains("blobs"), "{result}");
}

#[test]
fn pull_012_a_blob_whose_content_is_not_its_name_is_a_checksum() {
    let model = FoundModel {
        tool: Tool::Ollama,
        location: "x".to_owned(),
        name: "x:latest".to_owned(),
        root: "/r".to_owned(),
        files: vec![FoundFile {
            location: vec!["blobs".to_owned()],
            size_bytes: 5,
            named_sha256: Some(SHA_A.to_owned()),
        }],
        usable: true,
    };

    assert_eq!(named_hash_holds(&model, &[SHA_A.to_owned()]), Ok(()));
    assert_eq!(
        named_hash_holds(&model, &[SHA_B.to_owned()]),
        Err(Cause::Checksum)
    );
}

#[test]
fn pull_012_the_owner_adds_folders_and_can_turn_the_walk_off() {
    let settings =
        MachineSettings::parse(r#"{ "findModels": false, "modelFolders": ["/srv/models", "/"] }"#)
            .expect("settings");
    assert!(!settings.find_models);
    assert_eq!(settings.model_folders, ["/srv/models", "/"]);
    assert!(MachineSettings::default().find_models);

    for bad in [
        r#"{ "modelFolders": ["models"] }"#.to_owned(),
        r#"{ "modelFolders": ["/srv/../etc"] }"#.to_owned(),
        r#"{ "modelFolders": ["/srv//x"] }"#.to_owned(),
        format!(r#"{{ "modelFolders": {} }}"#, json!(vec!["/a"; 17])),
    ] {
        assert_eq!(
            MachineSettings::parse(&bad),
            Err(SettingsError::Value("modelFolders")),
            "{bad}"
        );
    }
}
