//! The commands and the skills of a program (TH117): untrusted names and words, cut to one
//! plain line, and a row whose name is not a plain key left out.

use super::*;

fn lists(events: &[DriveEvent]) -> &CommandLists {
    events
        .iter()
        .find_map(|event| match event {
            DriveEvent::Commands { lists } => Some(lists),
            _ => None,
        })
        .expect("the lists")
}

#[test]
fn th117_the_claude_commands_come_from_initialize_and_a_bad_name_is_left_out() {
    let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"anima-initialize","response":{"commands":[{"name":"review","description":"Review the\nchanges","argumentHint":"[file]"},{"name":"plugin:deploy","description":""},{"name":"rm -rf","description":"bad"},{"name":"../up","description":"bad"},{"name":"-flag","description":"bad"}],"account":{"email":"someone@example.com"}}}}"#;
    let events = claude(line);
    let lists = lists(&events);
    let names: Vec<&str> = lists.commands.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["review", "plugin:deploy"]);
    assert_eq!(
        lists.commands[0].description.as_deref(),
        Some("Review the changes")
    );
    assert_eq!(lists.commands[0].hint.as_deref(), Some("[file]"));
    assert!(lists.commands.iter().all(|row| row.run == Run::Text));
    assert!(
        lists.skills.is_empty(),
        "Claude Code names its skills as commands"
    );
    assert_eq!(lists.commands[1].description, None, "a blank description");
    assert!(!format!("{events:?}").contains("someone@example.com"));
}

#[test]
fn th117_a_description_is_cut_to_two_hundred_characters_and_the_list_to_two_hundred_rows() {
    let long = "x".repeat(500);
    let rows: Vec<Value> = (0..300)
        .map(|n| serde_json::json!({"name": format!("c{n}"), "description": long}))
        .collect();
    let answer = serde_json::json!({"commands": rows});
    let Some(DriveEvent::Commands { lists }) = claude_commands(&answer) else {
        panic!("the lists");
    };
    assert_eq!(lists.commands.len(), MOST_ROWS);
    let words = lists.commands[0].description.as_deref().expect("words");
    assert!(words.chars().count() <= 200, "{}", words.chars().count());
}

#[test]
fn th117_the_codex_skills_leave_out_a_skill_that_is_off_or_has_no_plain_path() {
    let line = r#"{"id":12,"result":{"data":[{"cwd":"/w","errors":[],"skills":[{"name":"review","description":"Review code","shortDescription":"Review","interface":{"shortDescription":"Reviews"},"enabled":true,"path":"/w/.codex/skills/review/SKILL.md","scope":"repo"},{"name":"off","description":"x","enabled":false,"path":"/s/off/SKILL.md","scope":"user"},{"name":"relative","description":"x","enabled":true,"path":"skills/x/SKILL.md","scope":"user"},{"name":"bad scope","description":"x","enabled":true,"path":"/s/x/SKILL.md","scope":"user"},{"name":"marked","description":"x","enabled":true,"path":"/s/\u202ex/SKILL.md","scope":"user"}]}]}}"#;
    let events = codex(line);
    assert!(matches!(events[0], DriveEvent::Reply { ok: true, .. }));
    let lists = lists(&events);
    assert!(lists.commands.is_empty(), "Codex lists only skills");
    assert_eq!(lists.skills.len(), 1);
    let skill = &lists.skills[0];
    assert_eq!(skill.name, "review");
    assert_eq!(
        skill.short.as_deref(),
        Some("Reviews"),
        "the interface first"
    );
    assert_eq!(skill.scope, "repo");
    assert_eq!(skill.path, "/w/.codex/skills/review/SKILL.md");
}

#[test]
fn th117_a_claude_command_file_gives_its_frontmatter_or_its_first_line() {
    let with_front =
        "---\ndescription: \"Make a release\"\nargument-hint: [version]\n---\nThe body.\n";
    let row = claude_file("release", with_front).expect("a row");
    assert_eq!(row.description.as_deref(), Some("Make a release"));
    assert_eq!(row.hint.as_deref(), Some("[version]"));
    let plain = claude_file("frontend:page", "\n# Make a page\u{202e}\nmore").expect("a row");
    assert_eq!(row.name, "release");
    assert_eq!(plain.name, "frontend:page");
    assert_eq!(
        plain.description.as_deref(),
        Some("Make a page"),
        "no direction mark"
    );
    let skill = claude_file("dir", "---\nname: tidy\ndescription: Tidy up\n---\n").expect("a row");
    assert_eq!(skill.name, "tidy", "a skill is named by its frontmatter");
    assert!(claude_file("bad name", "words").is_none());
    assert!(claude_file("dir", "---\nname: ../x\n---\n").is_none());
}
