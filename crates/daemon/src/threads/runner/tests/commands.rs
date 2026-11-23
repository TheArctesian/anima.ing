//! The skills and the compact of a Codex thread (TH117).

use std::time::{Duration, Instant};

use agent_threads::control::Refusal;
use agent_threads::drive::{ChosenSkill, Run};

use super::{codex_in_a_turn, counter, writes};

const SKILLS: &str = r#"{"id":6,"result":{"data":[{"cwd":"/w","errors":[],"skills":[{"name":"review","description":"Review","enabled":true,"path":"/w/.codex/skills/review/SKILL.md","scope":"repo"}]}]}}"#;

fn skill(name: &str, path: &str) -> ChosenSkill {
    ChosenSkill {
        name: name.to_owned(),
        path: path.to_owned(),
    }
}

#[test]
fn th117_a_codex_prompt_takes_only_a_skill_that_codex_listed_for_the_thread() {
    let mut ids = counter();
    let (mut thread, _) = codex_in_a_turn(&mut ids);
    let listed = skill("review", "/w/.codex/skills/review/SKILL.md");
    assert_eq!(
        thread.takes(std::slice::from_ref(&listed)),
        Err(Refusal::BadValue),
        "no list yet"
    );
    let start = Instant::now();
    let at = |seconds| start + Duration::from_secs(seconds);
    let (lists, again) = thread.commands(start);
    assert!(lists.is_none());
    assert_eq!(
        writes(&again)[0]["method"],
        "skills/list",
        "a thread with none asks again"
    );
    let (_, again) = thread.commands(at(5));
    assert!(again.is_empty(), "at most one ask in 10 seconds");
    thread.line(SKILLS, &mut ids);
    let (lists, again) = thread.commands(at(30));
    let lists = lists.expect("the lists");
    assert_eq!(lists.skills.len(), 1);
    assert_eq!(
        lists.commands[0].name, "compact",
        "the runner runs it for Codex"
    );
    assert_eq!(lists.commands[0].run, Run::Call);
    assert!(again.is_empty(), "a fresh list asks nothing");
    assert_eq!(thread.takes(std::slice::from_ref(&listed)), Ok(()));
    for other in [
        skill("review", "/home/you/.ssh/id_ed25519"),
        skill("other", "/w/.codex/skills/review/SKILL.md"),
    ] {
        assert_eq!(thread.takes(&[other]), Err(Refusal::BadValue));
    }
    assert_eq!(thread.takes(&[]), Ok(()));
    let (lists, again) = thread.commands(at(61));
    assert!(lists.is_some(), "the old list meanwhile");
    assert_eq!(
        writes(&again)[0]["method"],
        "skills/list",
        "a list older than a minute"
    );
}

#[test]
fn th117_a_codex_thread_compacts_with_its_own_call() {
    let mut ids = counter();
    let (mut thread, _) = codex_in_a_turn(&mut ids);
    let steps = thread.compact().expect("compacts");
    let sent = writes(&steps);
    assert_eq!(sent[0]["method"], "thread/compact/start");
    assert_eq!(sent[0]["params"]["threadId"], "th");
}
