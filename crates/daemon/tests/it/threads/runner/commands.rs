//! The calls `commands` and `compact` and the skills of a prompt (TH117), with a made Claude
//! Code that names its commands in its answer to `initialize`.

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::{ChosenSkill, CommandLists, Mode, Program};
use uuid::Uuid;

use super::{Client, Kit};

const CLAUDE_COMMANDS: &str = r#"#!/usr/bin/perl
$| = 1;
my $sid = "none";
for my $i (0 .. $#ARGV) { $sid = $ARGV[$i + 1] if $ARGV[$i] eq "--session-id"; }
print qq({"type":"system","subtype":"init","session_id":"$sid","model":"made","permissionMode":"default"}\n);
while (my $line = <STDIN>) {
    if ($line =~ /"subtype":"initialize"/ && $line =~ /"request_id":"([^"]+)"/) {
        print qq({"type":"control_response","response":{"subtype":"success","request_id":"$1","response":{"commands":[{"name":"review","description":"Review the changes","argumentHint":"[file]"},{"name":"rm -rf","description":"not a name"}]}}}\n);
    }
}
"#;

fn start(kit: &Kit) -> Action {
    Action::Start {
        program: Program::ClaudeCode,
        folder: kit.path("work"),
        mode: Mode::FullAccess,
        model: None,
        effort: None,
        speed: None,
        prompt: "Look".to_owned(),
        program_path: None,
        draft: None,
        attachments: Vec::new(),
        worktree: false,
        base: None,
    }
}

async fn commands(client: &mut Client, action: Action) -> (CommandLists, bool) {
    let id = client.call(action).await;
    match client.done(id).await {
        Outcome::Commands { lists, pending } => (lists, pending),
        other => panic!("the lists: {other:?}"),
    }
}

fn names(lists: &CommandLists) -> Vec<&str> {
    lists.commands.iter().map(|row| row.name.as_str()).collect()
}

#[tokio::test]
async fn th117_a_claude_thread_lists_the_commands_of_its_answer_to_initialize() {
    let kit = Kit::with(CLAUDE_COMMANDS);
    let mut client = kit.client().await;
    let started = client.call(start(&kit)).await;
    let Outcome::Started { thread, .. } = client.done(started).await else {
        panic!("the thread started");
    };
    let of_thread = Action::Commands {
        thread: Some(thread),
        program: None,
        folder: None,
    };
    let mut answer = commands(&mut client, of_thread.clone()).await;
    for _ in 0..50 {
        if !answer.1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        answer = commands(&mut client, of_thread.clone()).await;
    }
    let (lists, pending) = answer;
    assert!(!pending, "Claude Code named its commands");
    assert_eq!(
        names(&lists),
        ["review"],
        "a name that is not a key is left out"
    );
    assert_eq!(lists.commands[0].hint.as_deref(), Some("[file]"));

    let skill = ChosenSkill {
        name: "review".to_owned(),
        path: "/x/SKILL.md".to_owned(),
    };
    let send = client
        .call(Action::Send {
            thread,
            text: "go".to_owned(),
            attachments: Vec::new(),
            skills: vec![skill],
        })
        .await;
    assert_eq!(
        client.done(send).await,
        Outcome::Refused {
            why: Refusal::BadValue
        },
        "Claude Code takes a skill as the text /name, never as a path"
    );
    let compact = client.call(Action::Compact { thread }).await;
    assert!(matches!(
        client.done(compact).await,
        Outcome::Refused {
            why: Refusal::CannotChange { .. }
        }
    ));
    let unknown = Action::Commands {
        thread: Some(Uuid::nil()),
        program: None,
        folder: None,
    };
    let id = client.call(unknown).await;
    assert_eq!(
        client.done(id).await,
        Outcome::Refused {
            why: Refusal::UnknownThread
        }
    );
}

#[tokio::test]
async fn th117_a_folder_lists_the_claude_command_files_of_the_folder_then_of_the_home() {
    let kit = Kit::with(CLAUDE_COMMANDS);
    let write = |path: &str, text: &str| {
        let path = kit.path(path);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
        std::fs::write(path, text).expect("written");
    };
    write(
        "work/.claude/commands/deploy.md",
        "---\ndescription: Deploy it\nargument-hint: [where]\n---\nbody",
    );
    write("work/.claude/commands/front/page.md", "# Make a page\n");
    write("work/.claude/commands/bad name.md", "x");
    write(
        "work/.claude/skills/tidy/SKILL.md",
        "---\nname: tidy\ndescription: Tidy up\n---\n",
    );
    write("secret.txt", "a secret line\n");
    std::os::unix::fs::symlink(
        kit.path("secret.txt"),
        kit.path("work/.claude/commands/link.md"),
    )
    .expect("a link");
    write(
        ".claude/commands/deploy.md",
        "---\ndescription: The home one\n---\n",
    );
    write(".claude/commands/home-only.md", "From home\n");
    let mut client = kit.client().await;
    let of_folder = Action::Commands {
        thread: None,
        program: Some(Program::ClaudeCode),
        folder: Some(kit.path("work")),
    };
    let (lists, pending) = commands(&mut client, of_folder).await;
    assert!(!pending);
    assert_eq!(
        names(&lists),
        ["deploy", "front:page", "tidy", "home-only"],
        "no bad name, no link; the folder wins over the home"
    );
    assert_eq!(lists.commands[0].description.as_deref(), Some("Deploy it"));
    assert_eq!(
        lists.commands[1].description.as_deref(),
        Some("Make a page")
    );
    assert!(!format!("{lists:?}").contains("a secret line"));

    let daemon = Action::Commands {
        thread: None,
        program: Some(Program::ClaudeCode),
        folder: Some(kit.path("daemon")),
    };
    let id = client.call(daemon).await;
    assert_eq!(
        client.done(id).await,
        Outcome::Refused {
            why: Refusal::FolderNotPermitted
        },
        "never the folder of the daemon"
    );
    let both = Action::Commands {
        thread: Some(Uuid::nil()),
        program: Some(Program::ClaudeCode),
        folder: Some(kit.path("work")),
    };
    let id = client.call(both).await;
    assert_eq!(
        client.done(id).await,
        Outcome::Refused {
            why: Refusal::BadValue
        }
    );
}
