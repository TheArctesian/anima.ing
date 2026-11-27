use compose_rules::Rule;

use super::{
    APPLICATION, PROJECT, World,
    applying::{COMPOSE, ENVIRONMENT, digests},
    commands,
};
use crate::{
    Refusal, Step,
    work::back::{Kept, accepted, prepared},
};

const KEPT: Kept<'static> = Kept {
    compose: COMPOSE.as_bytes(),
    environment: ENVIRONMENT.as_bytes(),
};

fn with_a_kept_version() -> World {
    let mut world = World::new();
    world.records.applications[0].kept = Some(digests());
    world
}

#[test]
fn k10_6_4_9_an_application_with_no_kept_version_is_refused() {
    let world = World::new();
    assert_eq!(
        accepted(APPLICATION, &world.facts()),
        Err(Refusal::NothingKept)
    );
    assert_eq!(
        prepared(APPLICATION, &KEPT, &world.facts()).err(),
        Some(Refusal::NothingKept)
    );
    let unknown = "0192f6b0-7c1e-7a3b-9d2f-000000000000";
    assert_eq!(accepted(unknown, &world.facts()), Err(Refusal::NothingKept));
    assert_eq!(
        accepted(APPLICATION, &with_a_kept_version().facts()),
        Ok(())
    );
}

#[test]
fn k10_6_4_9_the_kept_version_starts_with_the_kept_digests_and_no_pull() {
    let world = with_a_kept_version();
    let planned = prepared(APPLICATION, &KEPT, &world.facts()).expect("a kept version");
    let commands = commands(&planned.plan);
    assert_eq!(commands.len(), 1, "no pull and no tag: one command");
    let verbs = commands[0].arguments[6..].join(" ");
    assert_eq!(verbs, "up -d --remove-orphans --pull never --no-build");
    assert_eq!(commands[0].arguments[1], PROJECT);
    // It does not bring back data, and the result says so.
    assert!(planned.holds_volumes);
}

#[test]
fn k10_6_4_9_going_back_does_not_keep_the_replaced_version() {
    let world = with_a_kept_version();
    let planned = prepared(APPLICATION, &KEPT, &world.facts()).expect("a kept version");
    assert!(
        planned.plan.steps.iter().any(
            |step| matches!(step, Step::ApplyVersion { version, .. } if !version.keep_previous)
        )
    );
}

#[test]
fn k10_6_4_9_the_kept_files_pass_the_rules_of_today() {
    let world = with_a_kept_version();
    let old = COMPOSE.replacen("    tmpfs: /run", "    privileged: true", 1);
    let kept = Kept {
        compose: old.as_bytes(),
        ..KEPT
    };
    assert_eq!(
        prepared(APPLICATION, &kept, &world.facts()).err(),
        Some(Refusal::FileNotPermitted {
            rule: Rule::KeyNotPermitted,
            service: Some(0)
        })
    );
    let mut world = with_a_kept_version();
    world.records.applications[0].kept = Some(digests()[..1].to_vec());
    assert_eq!(
        prepared(APPLICATION, &KEPT, &world.facts()).err(),
        Some(Refusal::NoDigest)
    );
}

#[test]
fn k10_6_4_9_a_back_has_the_refusals_of_an_apply_for_its_project() {
    let mut world = with_a_kept_version();
    world.busy = true;
    assert_eq!(accepted(APPLICATION, &world.facts()), Err(Refusal::Busy));
    world.busy = false;
    world.settings.protected = vec![PROJECT.to_owned()];
    assert_eq!(
        accepted(APPLICATION, &world.facts()),
        Err(Refusal::Protected)
    );
}
