//! The hand-out of model items (K11 Section 3): the device policy, the room of a claim and the
//! order.

use daemon_core::model_work::names::Actor;
use daemon_core::wire::{Free, Switch, WorkKind, WorkType};

use super::placement::{offer, verdicts};
use super::{MAC, PVE, at, claim_of, device, queued};
use crate::work::jobs::{
    model::{Job, State, Target},
    placement::{self, Offer, Room, Verdict},
};

fn model(work_type: WorkType) -> Job {
    let mut job = queued(work_type);
    job.asked.kind = WorkKind::Model;
    job.asked.os = None;
    job.asked.architecture = None;
    job.asked.target = Target::Model { actor: Actor::Hub };
    job
}

fn held(job: Job) -> Job {
    Job {
        state: State::Running,
        holder: Some(device(PVE)),
        ..job
    }
}

/// The offer of a machine that takes model items, with its policy.
fn model_offer(policy_permits: bool) -> Offer {
    let mut claim = claim_of(PVE);
    claim.kinds.push(WorkKind::Model);
    claim.types.extend(daemon_core::wire::MODEL_TYPES);
    Offer {
        policy_permits,
        ..offer(&claim)
    }
}

/// The claim of a daemon that offers only model work, as `Node::claim` of the daemon writes it: no
/// job slot and no room (K11 Section 3).
fn model_only_offer() -> Offer {
    let mut claim = claim_of(PVE);
    claim.kinds = vec![WorkKind::Model];
    claim.types = daemon_core::wire::MODEL_TYPES.to_vec();
    claim.free = Free {
        slots: 0,
        memory_bytes: 0,
        disk_bytes: 0,
    };
    Offer {
        policy_permits: true,
        ..offer(&claim)
    }
}

#[test]
fn k11_3_each_model_item_goes_to_a_daemon_that_offers_no_room_of_free() {
    let daemon = model_only_offer();
    for work_type in daemon_core::wire::MODEL_TYPES {
        let job = model(work_type);
        assert!(job.asked.limits.memory_bytes > 0 && job.asked.limits.disk_bytes > 0);
        let decided = verdicts(&[job], &daemon, &[], at(3600));
        assert_eq!(decided, [Verdict::Fits], "{work_type:?}");
    }
}

#[test]
fn a_job_of_k10_still_needs_the_room_that_it_asks() {
    let mut claim = claim_of(PVE);
    claim.free.memory_bytes = 0;
    let decided = verdicts(&[queued(WorkType::Build)], &offer(&claim), &[], at(3600));
    assert_eq!(decided, [Verdict::Memory]);
}

#[test]
fn cap_010_a_policy_that_stops_the_node_stops_each_model_item_but_the_reducing_ones() {
    let stopped = model_offer(false);
    let reducing = [
        WorkType::ServerStop,
        WorkType::ModelUnload,
        WorkType::ModelDrain,
        WorkType::CopyDelete,
        WorkType::ModelReport,
    ];
    for work_type in daemon_core::wire::MODEL_TYPES {
        let expected = if reducing.contains(&work_type) {
            Verdict::Fits
        } else {
            Verdict::Policy
        };
        let decided = verdicts(&[model(work_type)], &stopped, &[], at(3600));
        assert_eq!(decided, [expected], "{work_type:?}");
    }
    let permitted = verdicts(
        &[model(WorkType::ServerStart)],
        &model_offer(true),
        &[],
        at(3600),
    );
    assert_eq!(permitted, [Verdict::Fits]);
    // The policy is of model work only: a job of K10 still goes to the node.
    let build = verdicts(&[queued(WorkType::Build)], &stopped, &[], at(3600));
    assert_eq!(build, [Verdict::Fits]);
}

#[test]
fn a_model_item_takes_no_job_slot_and_one_change_runs_at_a_time() {
    let mut full = claim_of(PVE);
    full.free.slots = 0;
    let room = Room::of(&full, &[]);
    assert!(room.admits(&model(WorkType::ServerStart)));
    assert!(!room.admits(&queued(WorkType::Build)));

    let room = Room::of(&claim_of(PVE), &[held(queued(WorkType::Build))]);
    assert!(
        room.admits(&model(WorkType::ModelPull)),
        "a held job leaves model items"
    );
    assert!(!room.admits(&queued(WorkType::Runner)));

    let room = Room::of(&claim_of(PVE), &[held(model(WorkType::ModelPull))]);
    assert!(!room.admits(&model(WorkType::ServerStart)));
    for free in [
        WorkType::ModelDrain,
        WorkType::ModelReport,
        WorkType::ModelMeasure,
    ] {
        assert!(room.admits(&model(free)), "{free:?}");
    }
    assert!(
        room.admits(&queued(WorkType::Build)),
        "a held model item leaves the slot"
    );

    let room = Room::of(&claim_of(PVE), &[held(model(WorkType::ModelMeasure))]);
    assert!(!room.admits(&model(WorkType::ModelMeasure)));
    assert!(room.admits(&model(WorkType::ServerStart)));
}

#[test]
fn a_machine_whose_switch_is_off_has_no_room() {
    let mut off = claim_of(MAC);
    off.switch = Switch::Off;
    let room = Room::of(&off, &[]);
    assert!(room.is_empty());
    assert!(!room.admits(&model(WorkType::ModelDrain)));
}

#[test]
fn a_claim_gets_a_drain_then_a_stop_then_an_unload_then_other_model_items_then_a_job() {
    let mut queue = [
        queued(WorkType::Build),
        model(WorkType::ModelPull),
        model(WorkType::ModelUnload),
        model(WorkType::ServerStop),
        model(WorkType::ModelDrain),
    ];
    queue.sort_by_key(placement::rank);
    let order: Vec<_> = queue.iter().map(|job| job.asked.work_type).collect();
    assert_eq!(
        order,
        [
            WorkType::ModelDrain,
            WorkType::ServerStop,
            WorkType::ModelUnload,
            WorkType::ModelPull,
            WorkType::Build,
        ]
    );
}

#[test]
fn a_drain_that_this_machine_held_last_still_goes_before_a_job() {
    let drain = Job {
        last_device: Some(device(PVE)),
        ..model(WorkType::ModelDrain)
    };
    let queue = [drain, queued(WorkType::Build)];
    let fits = [Verdict::Fits, Verdict::Fits];
    let chosen = placement::choose(&queue, &fits, device(PVE)).map(|job| job.asked.work_type);
    assert_eq!(chosen, Some(WorkType::ModelDrain));
}

mod reports {
    use daemon_core::wire::{
        Cleanup, CopyOutcome, ModelOutcome, Progress, WorkReport, WorkResult, WorkState, WorkType,
    };

    use super::{PVE, at, device, model, queued};
    use crate::work::jobs::{
        lifecycle::{self, Refusal},
        model::{Job, State},
    };

    fn holding(job: &Job, state: State) -> Job {
        Job {
            state,
            ..lifecycle::claimed(job, device(PVE), at(0))
        }
    }

    fn result(cause: Option<&str>, copy_state: Option<&str>) -> WorkResult {
        WorkResult {
            outcome: if cause.is_some() {
                "failed"
            } else {
                "succeeded"
            }
            .to_owned(),
            cause: cause.map(str::to_owned),
            seconds: 3,
            cleanup: Cleanup {
                isolation: "deleted".to_owned(),
                checkout: "deleted".to_owned(),
                secrets: "deleted".to_owned(),
                keychain: "none".to_owned(),
            },
            model: copy_state.map(|state| {
                ModelOutcome::Copy(CopyOutcome {
                    copy: uuid::Uuid::now_v7(),
                    copy_state: state.to_owned(),
                    bytes: 10,
                    files: 1,
                    file_hashes: Vec::new(),
                })
            }),
        }
    }

    fn report(state: WorkState, result: Option<WorkResult>, bytes: u64) -> WorkReport {
        WorkReport {
            id: super::super::JOB.parse().unwrap(),
            attempt: 1,
            state,
            log: None,
            result,
            progress: (bytes > 0).then_some(Progress {
                bytes,
                total_bytes: 100,
                bytes_per_second: 10,
            }),
        }
    }

    fn reported(job: &Job, sent: &WorkReport) -> Result<Job, Refusal> {
        lifecycle::reported(job, device(PVE), sent, at(10))
    }

    #[test]
    fn pull_007_a_running_pull_keeps_its_last_progress() {
        let running = holding(&model(WorkType::ModelPull), State::Running);
        let first = reported(&running, &report(WorkState::Running, None, 40)).unwrap();
        assert_eq!(first.progress.map(|progress| progress.bytes), Some(40));
        let next = reported(&first, &report(WorkState::Running, None, 0)).unwrap();
        assert_eq!(next.progress, first.progress);
    }

    #[test]
    fn pull_007_progress_is_only_for_a_running_pull_within_its_total() {
        let running = holding(&model(WorkType::ModelPull), State::Running);
        let over = reported(&running, &report(WorkState::Running, None, 101));
        assert_eq!(over, Err(Refusal::Malformed));
        let build = holding(&queued(WorkType::Build), State::Running);
        let on_build = reported(&build, &report(WorkState::Running, None, 40));
        assert_eq!(on_build, Err(Refusal::Malformed));
        let preparing = holding(&model(WorkType::ModelPull), State::Preparing);
        let early = reported(&preparing, &report(WorkState::Preparing, None, 40));
        assert_eq!(early, Err(Refusal::Malformed));
    }

    #[test]
    fn k11_3_a_model_result_has_a_copy_state_of_the_list_and_only_on_a_model_item() {
        let finishing = holding(&model(WorkType::ModelPull), State::Finishing);
        let done = report(WorkState::Done, Some(result(None, Some("downloaded"))), 0);
        let kept = reported(&finishing, &done).unwrap();
        assert_eq!(
            kept.result.unwrap().copy().unwrap().copy_state,
            "downloaded"
        );
        let strange = report(WorkState::Done, Some(result(None, Some("loaded"))), 0);
        assert_eq!(reported(&finishing, &strange), Err(Refusal::Malformed));
        let build = holding(&queued(WorkType::Build), State::Finishing);
        let on_build = report(WorkState::Done, Some(result(None, Some("downloaded"))), 0);
        assert_eq!(reported(&build, &on_build), Err(Refusal::Malformed));
    }

    #[test]
    fn k11_5_a_model_item_ends_with_the_cause_of_its_refusal_but_policy_and_switch_off() {
        let claimed = holding(&model(WorkType::ModelPull), State::Claimed);
        let full = report(
            WorkState::Refused,
            Some(result(Some("disk_share"), None)),
            0,
        );
        let failed = reported(&claimed, &full).unwrap();
        assert_eq!(failed.state, State::Failed);
        assert_eq!(failed.reason.as_deref(), Some("disk_share"));
        assert_eq!(failed.refused_by.len(), 0);
        for cause in ["policy", "switch_off"] {
            let later = report(WorkState::Refused, Some(result(Some(cause), None)), 0);
            let again = reported(&claimed, &later).unwrap();
            assert_eq!(
                (again.state, again.holder),
                (State::Queued, None),
                "{cause}"
            );
            assert!(again.refused_by.is_empty(), "{cause}");
        }
    }
}
