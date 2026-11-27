use super::{Import, Incoming, Phase, Record, Stage, matching, set_stage};
use agent_threads::managed::workspace::handoff::{Binding, Receipt, Source};
use uuid::Uuid;

fn import_fixture() -> Import {
    let source = Source {
        workspace: Uuid::from_u128(1),
        principal: Uuid::from_u128(2),
        generation: 7,
        binding: Binding {
            transfer: Uuid::from_u128(3),
            destination: Uuid::from_u128(4),
        },
    };
    Import {
        source_device: Uuid::from_u128(5),
        destination_device: source.binding.destination,
        storage_bytes: 1 << 30,
        checkout: None,
        receipt: Receipt {
            schema: 1,
            source,
            digest: format!("sha256:{}", "a".repeat(64)),
            bytes: 1536,
            entries: 0,
        },
    }
}

#[test]
fn restored_destination_requires_exact_activation_before_becoming_launchable() {
    let request = import_fixture();
    let source = request.receipt.source;
    assert!(request.fits());
    let mut state = Record::new(source.workspace, request.storage_bytes);
    state.principal = Some(source.principal);
    state.generation = source.generation;
    state.incoming = Some(Incoming {
        import: request.clone(),
        stage: Stage::Restoring,
    });
    assert!(state.require_launchable().is_err());
    set_stage(&mut state, Stage::Restored).unwrap();
    let mut restored: Record =
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    assert!(
        matching(&restored, &request)
            .unwrap()
            .validate(&restored)
            .is_ok()
    );
    let other = Import {
        source_device: Uuid::from_u128(6),
        ..request.clone()
    };
    assert!(matching(&restored, &other).is_err());
    assert!(restored.require_launchable().is_err());
    set_stage(&mut restored, Stage::Activated).unwrap();
    assert!(
        matching(&restored, &request)
            .unwrap()
            .validate(&restored)
            .is_err()
    );
    restored.phase = Phase::Prepared;
    assert!(
        matching(&restored, &request)
            .unwrap()
            .validate(&restored)
            .is_ok()
    );
    assert!(restored.require_launchable().is_ok());
    assert_eq!(restored.generation + 1, 8);
}

#[test]
fn return_requires_a_retired_source_and_a_newer_generation_of_the_same_workspace() {
    let returning = import_fixture();
    let source = returning.receipt.source;
    let mut old = Record::new(source.workspace, 1 << 30);
    old.principal = Some(source.principal);
    old.generation = source.generation - 1;
    old.handoff = Some(Binding {
        transfer: Uuid::from_u128(8),
        destination: returning.source_device,
    });
    old.phase = Phase::Stopped;
    assert!(!super::returning_to_retired_source(&old, &returning));
    old.phase = Phase::Deleted;
    old.storage_bytes = 0;
    assert!(super::returning_to_retired_source(&old, &returning));
    old.generation = source.generation;
    assert!(!super::returning_to_retired_source(&old, &returning));
    old.generation = source.generation - 1;
    old.principal = Some(Uuid::from_u128(9));
    assert!(!super::returning_to_retired_source(&old, &returning));
}
