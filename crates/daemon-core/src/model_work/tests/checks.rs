use serde_json::{Value, json};

use crate::model_work::cause::Cause;
use crate::model_work::machine::MachineSettings;
use crate::model_work::order::{Checked, ModelMachine, Parameters, check};
use crate::model_work::settings::{OWNED, SettingLimits};
use crate::wire::{MODEL_TYPES, WorkType};

const ID: &str = "0192f6aa-0000-7000-8000-000000000001";
const SERVER: &str = "0192f6aa-0000-7000-8000-000000000002";

fn item(work_type: &str, parameters: &Value) -> Value {
    json!({
        "id": ID, "type": work_type, "kind": "model", "attempt": 1,
        "lease": { "until": "2026-11-20T10:01:30Z" },
        "limits": { "seconds": 600, "idleSeconds": 60, "memoryBytes": 1, "diskBytes": 1, "orphanSeconds": 60 },
        "project": null, "secrets": [], "parameters": parameters, "actor": "User",
    })
}

fn start(settings: &Value) -> Value {
    item(
        "server_start",
        &json!({ "server": SERVER, "runtime": "llama_server", "version": "b8100", "backend": "metal", "settings": settings }),
    )
}

fn checked_with(answer: &Value, settings: &MachineSettings, switch_on: bool) -> Checked {
    let machine = ModelMachine {
        switch_on,
        policy_permits: true,
        settings,
        limits: SettingLimits {
            memory_share: 0.5,
            gpus: &[0],
        },
    };
    check(answer, &machine)
}

fn cause_of(answer: &Value) -> Option<Cause> {
    match checked_with(answer, &MachineSettings::default(), true) {
        Checked::Refuse { cause, .. } => Some(cause),
        Checked::Start(_) | Checked::Drop => None,
    }
}

#[test]
fn a_type_outside_the_list_is_refused() {
    for name in ["build", "runner", "shell", "model_exec", ""] {
        assert_eq!(
            cause_of(&item(name, &json!({}))),
            Some(Cause::TypeUnknown),
            "{name}"
        );
    }
    assert_eq!(MODEL_TYPES.len(), 12);
}

#[test]
fn a_whole_server_start_passes() {
    let answer =
        start(&json!({ "contextTokens": 8192, "gpuLayers": "all", "kvCacheType": "q8_0" }));
    let Checked::Start(order) = checked_with(&answer, &MachineSettings::default(), true) else {
        panic!("refused");
    };
    assert!(matches!(order.parameters, Parameters::Start { .. }));
}

#[test]
fn each_owned_flag_is_refused() {
    for name in OWNED
        .iter()
        .copied()
        .chain(["--host", "-p", "--trust-remote-code"])
    {
        let answer = start(&json!({ name: "x" }));
        assert_eq!(cause_of(&answer), Some(Cause::FlagOwned), "{name}");
    }
}

#[test]
fn a_member_that_the_runtime_does_not_take_is_refused() {
    assert_eq!(
        cause_of(&start(&json!({ "memoryShare": 0.2 }))),
        Some(Cause::MemberUnknown)
    );
    assert_eq!(
        cause_of(&start(&json!({ "contextTokens": 10 }))),
        Some(Cause::MemberUnknown)
    );
    assert_eq!(
        cause_of(&start(&json!({ "gpuLayers": "most" }))),
        Some(Cause::MemberUnknown)
    );
}

#[test]
fn an_item_with_a_repository_or_an_extra_member_is_refused() {
    let mut answer = item("model_report", &json!({}));
    answer["repository"] = json!({ "forge": "github", "owner": "a", "name": "b" });
    assert_eq!(cause_of(&answer), Some(Cause::MemberUnknown));
    assert_eq!(
        cause_of(&item("model_report", &json!({ "path": "/" }))),
        Some(Cause::MemberUnknown)
    );
    let mut answer = item("model_report", &json!({}));
    answer["kind"] = json!("native");
    assert_eq!(cause_of(&answer), Some(Cause::KindNotPermitted));
}

#[test]
fn the_actor_check_follows_agent_types() {
    let settings = MachineSettings {
        agent_types: vec![WorkType::ModelReport, WorkType::ServerStop],
        ..MachineSettings::default()
    };
    let mut answer = start(&json!({}));
    answer["actor"] = json!("Agent");
    let refused = checked_with(&answer, &settings, true);
    assert!(matches!(
        refused,
        Checked::Refuse {
            cause: Cause::ActorNotPermitted,
            ..
        }
    ));
    answer["actor"] = json!("User");
    assert!(matches!(
        checked_with(&answer, &settings, true),
        Checked::Start(_)
    ));
    let mut stop = item("server_stop", &json!({ "server": SERVER }));
    stop["actor"] = json!("Agent");
    assert!(matches!(
        checked_with(&stop, &settings, true),
        Checked::Start(_)
    ));
}

#[test]
fn a_reducing_type_passes_with_the_switch_off() {
    let stop = item("server_stop", &json!({ "server": SERVER }));
    assert!(matches!(
        checked_with(&stop, &MachineSettings::default(), false),
        Checked::Start(_)
    ));
    let refused = checked_with(&start(&json!({})), &MachineSettings::default(), false);
    assert!(matches!(
        refused,
        Checked::Refuse {
            cause: Cause::SwitchOff,
            ..
        }
    ));
}

fn pull(organisation: &str, name: &str) -> Value {
    item(
        "model_pull",
        &json!({
            "copy": SERVER, "variant": SERVER,
            "source": { "organisation": organisation, "repository": "Qwen3-8B-GGUF", "revision": "a".repeat(40) },
            "format": "gguf", "files": [ { "name": name, "sizeBytes": 5, "sha256": "b".repeat(64) } ], "gated": false,
        }),
    )
}

#[test]
fn a_pull_checks_its_files_and_its_organisation() {
    assert_eq!(cause_of(&pull("unsloth", "Q4.gguf")), None);
    for name in ["model.bin", "sub/x.pkl", "run.py", "weights.safetensors"] {
        assert_eq!(
            cause_of(&pull("unsloth", name)),
            Some(Cause::FileType),
            "{name}"
        );
    }
    for name in ["../x.gguf", "/x.gguf", "a//b.gguf", "a/./b.gguf"] {
        assert_eq!(
            cause_of(&pull("unsloth", name)),
            Some(Cause::MemberUnknown),
            "{name}"
        );
    }
    assert_eq!(
        cause_of(&pull("someone", "Q4.gguf")),
        Some(Cause::SourceNotPermitted)
    );
}

#[test]
fn an_answer_with_no_id_is_dropped() {
    let mut answer = item("model_report", &json!({}));
    answer["attempt"] = json!(0);
    assert_eq!(
        checked_with(&answer, &MachineSettings::default(), true),
        Checked::Drop
    );
}

#[test]
fn r_408_two_files_that_flatten_to_one_name_are_refused() {
    let file = |name: &str| json!({ "name": name, "sizeBytes": 5, "sha256": "b".repeat(64) });
    let mut both = pull("unsloth", "a/Q4.gguf");
    both["parameters"]["files"] = json!([file("a/Q4.gguf"), file("b/q4.GGUF")]);
    assert_eq!(cause_of(&both), Some(Cause::MemberUnknown));
    both["parameters"]["files"] = json!([file("a/Q4.gguf"), file("b/Q5.gguf")]);
    assert_eq!(cause_of(&both), None);
}

fn load(extra: &Value) -> Value {
    let mut parameters = json!({ "server": SERVER, "copy": ID, "settings": {} });
    if let (Some(object), Some(more)) = (parameters.as_object_mut(), extra.as_object()) {
        object.extend(more.clone());
    }
    item("model_load", &parameters)
}

#[test]
fn a_load_takes_an_optional_reservation_and_keep_and_nothing_else() {
    let whole = load(
        &json!({ "reserveBytes": 7_516_192_768_u64, "keep": { "idleSeconds": 900, "pinned": true } }),
    );
    let Checked::Start(order) = checked_with(&whole, &MachineSettings::default(), true) else {
        panic!("refused");
    };
    let Parameters::Load {
        reserve_bytes,
        keep,
        ..
    } = order.parameters
    else {
        panic!("not a load");
    };
    assert_eq!(reserve_bytes, Some(7_516_192_768));
    assert!(keep.is_some_and(|keep| keep.pinned && keep.idle_seconds == 900));
    assert_eq!(cause_of(&load(&json!({}))), None);
    for wrong in [
        json!({ "reserveBytes": 0 }),
        json!({ "reserveBytes": 9_007_199_254_740_992_u64 }),
        json!({ "keep": { "idleSeconds": 59, "pinned": false } }),
        json!({ "keep": { "idleSeconds": 86_401, "pinned": false } }),
        json!({ "keep": { "idleSeconds": 900 } }),
        json!({ "keep": { "idleSeconds": 900, "pinned": "yes" } }),
        json!({ "keep": { "idleSeconds": 900, "pinned": false, "priority": 1 } }),
        json!({ "evict": true }),
    ] {
        assert_eq!(
            cause_of(&load(&wrong)),
            Some(Cause::MemberUnknown),
            "{wrong}"
        );
    }
}

#[test]
fn a_keep_that_ends_a_pin_passes_with_the_switch_off_and_one_that_sets_a_pin_does_not() {
    let keep = |pinned: bool| {
        item(
            "model_keep",
            &json!({ "copy": ID, "keep": { "idleSeconds": 600, "pinned": pinned } }),
        )
    };
    let off = |answer: &Value| checked_with(answer, &MachineSettings::default(), false);
    assert!(matches!(off(&keep(false)), Checked::Start(_)));
    assert!(matches!(
        off(&keep(true)),
        Checked::Refuse {
            cause: Cause::SwitchOff,
            ..
        }
    ));
    let extra = item(
        "model_keep",
        &json!({ "copy": ID, "keep": { "idleSeconds": 600, "pinned": true }, "server": SERVER }),
    );
    assert_eq!(cause_of(&extra), Some(Cause::MemberUnknown));
}
