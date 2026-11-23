//! The hand-out of model items (K11 Section 3): the device policy, the room of a claim and the
//! order.

use daemon_core::model_work::names::Actor;
use daemon_core::wire::{Switch, WorkKind, WorkType};

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
