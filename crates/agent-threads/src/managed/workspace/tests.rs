use super::{Action, Command, Phase, Receipt, View};
use uuid::Uuid;

#[test]
fn deletion_receipt_requires_confirmation_exact_generation_and_reclaimed_storage() {
    let workspace = Uuid::now_v7();
    let mut receipt = Receipt {
        command: Command {
            request: Uuid::now_v7(),
            workspace,
            principal: Uuid::now_v7(),
            action: Action::Delete {
                generation: 0,
                confirmed: true,
            },
        },
        result: Some(View {
            workspace,
            generation: 0,
            phase: Phase::Deleted,
            storage_bytes: 0,
            checkout: None,
        }),
        failure: None,
    };
    assert!(
        receipt.fits(),
        "Never-started workspaces must be reclaimable"
    );
    receipt.command.action = Action::Delete {
        generation: 0,
        confirmed: false,
    };
    assert!(!receipt.fits());
    receipt.command.action = Action::Delete {
        generation: 1,
        confirmed: true,
    };
    assert!(!receipt.fits());
    receipt.command.action = Action::Delete {
        generation: 0,
        confirmed: true,
    };
    let view = receipt.result.as_mut().unwrap();
    view.phase = Phase::Stopped;
    view.storage_bytes = 1 << 30;
    assert!(!receipt.fits(), "Stopping does not prove deletion");
    receipt.result.as_mut().unwrap().phase = Phase::Deleted;
    assert!(!receipt.fits(), "Deletion must reclaim its quota");
    receipt.result.as_mut().unwrap().storage_bytes = 0;
    assert!(receipt.fits());
    receipt.command.action = Action::Start {
        generation: 1,
        memory_bytes: 1 << 30,
        cpu_millis: 500,
        tasks: 128,
    };
    receipt.result.as_mut().unwrap().generation = 1;
    assert!(
        !receipt.fits(),
        "Deletion is not a successful start receipt"
    );
}

#[test]
fn prepared_source_is_bound_to_the_exact_commit_and_cannot_run_before_checkout() {
    use super::{Checkout, Source};
    let workspace = Uuid::now_v7();
    let source = Source {
        owner: "example".into(),
        repository: "app".into(),
        commit: "a".repeat(40),
        repository_id: Some(Uuid::from_u128(7)),
    };
    let mut receipt = Receipt {
        command: Command {
            request: Uuid::now_v7(),
            workspace,
            principal: Uuid::now_v7(),
            action: Action::Prepare {
                storage_bytes: 1 << 30,
                source: Some(source.clone()),
            },
        },
        result: Some(View {
            workspace,
            phase: Phase::Prepared,
            generation: 0,
            storage_bytes: 1 << 30,
            checkout: Some(Checkout {
                source: Source {
                    commit: "b".repeat(40),
                    ..source.clone()
                },
                ready: false,
            }),
        }),
        failure: None,
    };
    assert!(!receipt.fits(), "Wrong commit");
    let mut wrong_repository = source.clone();
    wrong_repository.repository_id = None;
    receipt.result.as_mut().unwrap().checkout = Some(Checkout {
        source: wrong_repository,
        ready: false,
    });
    assert!(!receipt.fits(), "Missing private repository binding");
    let view = receipt.result.as_mut().unwrap();
    view.checkout = Some(Checkout {
        source: source.clone(),
        ready: false,
    });
    assert!(receipt.fits());
    receipt.command.action = Action::Start {
        generation: 1,
        memory_bytes: 1 << 30,
        cpu_millis: 500,
        tasks: 128,
    };
    let view = receipt.result.as_mut().unwrap();
    view.generation = 1;
    view.phase = Phase::Running;
    assert!(!receipt.fits(), "Checkout incomplete");
    let view = receipt.result.as_mut().unwrap();
    view.checkout = Some(Checkout {
        source,
        ready: true,
    });
    assert!(receipt.fits());
}

#[test]
fn reconciliation_can_confirm_that_the_requested_generation_never_started() {
    let workspace = Uuid::now_v7();
    let mut receipt = Receipt {
        command: Command {
            request: Uuid::now_v7(),
            workspace,
            principal: Uuid::now_v7(),
            action: Action::Reconcile { generation: 1 },
        },
        result: Some(View {
            workspace,
            phase: Phase::Prepared,
            generation: 0,
            storage_bytes: 1 << 30,
            checkout: None,
        }),
        failure: None,
    };
    assert!(receipt.fits());
    receipt.command.action = Action::Reconcile { generation: 2 };
    assert!(!receipt.fits());
    receipt.command.action = Action::Reconcile { generation: 1 };
    receipt.result.as_mut().unwrap().phase = Phase::Running;
    assert!(!receipt.fits());
}
