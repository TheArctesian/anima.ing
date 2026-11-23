use serde_json::{Value, json};

use crate::model_work::cause::Cause;
use crate::model_work::machine::MachineSettings;
use crate::model_work::order::{Checked, ModelMachine, Parameters, check};
use crate::model_work::settings::SettingLimits;
use crate::wire::WorkType;

fn drain(seconds: &Value, actor: &str) -> Value {
    json!({
        "id": "0192f6aa-0000-7000-8000-000000000001", "type": "model_drain", "kind": "model",
        "attempt": 1, "lease": { "until": "2026-11-20T10:01:30Z" },
        "limits": { "seconds": 600, "idleSeconds": 60, "memoryBytes": 1, "diskBytes": 1, "orphanSeconds": 60 },
        "project": null, "secrets": [], "parameters": { "seconds": seconds }, "actor": actor,
    })
}

fn checked(answer: &Value, settings: &MachineSettings, switch_on: bool, policy: bool) -> Checked {
    let machine = ModelMachine {
        switch_on,
        policy_permits: policy,
        settings,
        limits: SettingLimits {
            memory_share: 0.5,
            gpus: &[],
        },
    };
    check(answer, &machine)
}

#[test]
fn seconds_outside_1_to_60_or_not_an_integer_are_refused() {
    let settings = MachineSettings::default();
    for seconds in [
        json!(0),
        json!(61),
        json!(1_u64 << 40),
        json!(1.5),
        json!("60"),
    ] {
        assert!(
            matches!(
                checked(&drain(&seconds, "User"), &settings, true, true),
                Checked::Refuse {
                    cause: Cause::MemberUnknown,
                    ..
                }
            ),
            "{seconds}"
        );
    }
}

#[test]
fn a_drain_runs_with_the_switch_off_or_the_policy_against_work() {
    let settings = MachineSettings::default();
    for (switch_on, policy) in [(false, true), (true, false), (false, false)] {
        let Checked::Start(order) =
            checked(&drain(&json!(60), "User"), &settings, switch_on, policy)
        else {
            panic!("refused with switch {switch_on} and policy {policy}");
        };
        assert_eq!(order.parameters, Parameters::Drain { seconds: 60 });
    }
}

#[test]
fn an_agent_types_list_without_model_drain_refuses_an_agent_drain() {
    let settings = MachineSettings {
        agent_types: vec![WorkType::ModelReport],
        ..MachineSettings::default()
    };
    assert!(matches!(
        checked(&drain(&json!(60), "Agent"), &settings, true, true),
        Checked::Refuse {
            cause: Cause::ActorNotPermitted,
            ..
        }
    ));
}
