use super::{Binding, Phase, Record, record_intent, validate_record, view};
use uuid::Uuid;

#[test]
fn accepted_fence_survives_record_roundtrip_and_only_its_own_replay() {
    let workspace = Uuid::from_u128(1);
    let owner = Uuid::from_u128(2);
    let run = Uuid::from_u128(3);
    let binding = Binding {
        transfer: Uuid::from_u128(4),
        destination: Uuid::from_u128(5),
    };
    let mut state = Record::new(workspace, 1 << 30);
    state.principal = Some(owner);
    state.run = Some(run);
    state.generation = 7;
    state.phase = Phase::Running;

    assert!(record_intent(&mut state, 6, owner, binding).is_err());
    assert!(record_intent(&mut state, 7, Uuid::from_u128(9), binding).is_err());
    assert!(state.handoff.is_none());
    assert!(record_intent(&mut state, 7, owner, binding).unwrap());
    let mut recovered: Record =
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    assert!(validate_record(binding, &recovered).is_ok());
    assert!(recovered.require_launchable().is_err());
    assert_eq!(view(&recovered, binding)["state"], "stopping");
    assert_eq!(view(&recovered, binding)["run"], run.to_string());
    assert!(!record_intent(&mut recovered, 7, owner, binding).unwrap());
    let redirected = Binding {
        destination: Uuid::from_u128(6),
        ..binding
    };
    assert!(record_intent(&mut recovered, 7, owner, redirected).is_err());

    recovered.phase = Phase::Stopped;
    assert_eq!(view(&recovered, binding)["state"], "fenced");
    assert!(recovered.require_launchable().is_err());
    assert!(!record_intent(&mut recovered, 7, owner, binding).unwrap());
    assert_eq!(recovered.storage_bytes, 1 << 30);
}
