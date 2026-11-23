//! The commands and the skills of a program, for the `/` and `$` lists of the composer (TH117,
//! TC4S). Their names and words come from files of the person and of a repository: each is cut
//! to one plain line, and a row whose name is not a plain key is left out. Nothing here runs a
//! command: the person chooses one, and it goes as text or as a skill of a prompt. Pure.

use std::ops::Not;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{key, line, list};
use crate::drive::DriveEvent;
use crate::marks::is_hidden_mark;

/// The most rows of one list.
pub const MOST_ROWS: usize = 200;
/// The most bytes of the path of a skill.
const MOST_PATH: usize = 1024;
/// The bytes of a name beside letters, digits, `_` and `-`: a command of a plugin is
/// `plugin:command`.
const NAME_MORE: &[u8] = b":";

/// A command of the program, or of the runner for the program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRow {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What the command takes after its name, such as `[file]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub run: Run,
    /// It sends data of the thread off this computer: the client asks the person first.
    #[serde(default, skip_serializing_if = "Not::not")]
    pub leaves_computer: bool,
}

/// How a command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Run {
    /// The person sends it as the text `/name args` of a prompt (Claude Code).
    Text,
    /// A call of the runner of its own, named as the command (Codex `compact`).
    Call,
}

impl CommandRow {
    /// The command `compact` of a Codex thread: the call `compact` (TH117).
    #[must_use]
    pub fn compact() -> Self {
        Self {
            name: "compact".to_owned(),
            description: Some("Compact the context of the thread".to_owned()),
            hint: None,
            run: Run::Call,
            leaves_computer: false,
        }
    }
}

/// A skill of the program that a prompt names (Codex). Claude Code lists no skill here: it names
/// its skills and the commands of the person as commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRow {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The short words for a narrow row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    /// Where the skill comes from: `user`, `repo`, `system` or `admin`.
    pub scope: String,
    /// The file of the skill, as the program names it: a prompt names the skill with it.
    pub path: String,
    /// Only the person can use it: the agent does not choose it by itself (`userInvocationOnly`
    /// of Codex, when Codex gives it; Codex 0.170 does not).
    #[serde(default, skip_serializing_if = "Not::not")]
    pub user_only: bool,
}

/// The commands and the skills of a program, as it last named them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandLists {
    pub commands: Vec<CommandRow>,
    pub skills: Vec<SkillRow>,
}

/// The list `commands` of the answer of Claude Code to `initialize`, or `None` when the answer
/// has none. Claude Code lists its skills there too, each as a command.
#[must_use]
pub fn claude_commands(answer: &Value) -> Option<DriveEvent> {
    let commands = answer.get("commands")?.as_array()?;
    let commands = commands
        .iter()
        .filter_map(|command| {
            Some(CommandRow {
                name: key(&command["name"], NAME_MORE)?,
                description: line(&command["description"]),
                hint: line(&command["argumentHint"]),
                run: Run::Text,
                leaves_computer: false,
            })
        })
        .take(MOST_ROWS)
        .collect();
    Some(DriveEvent::Commands {
        lists: CommandLists {
            commands,
            skills: Vec::new(),
        },
    })
}

/// The answer to `skills/list` of Codex, or `None` when `result` is not one. A skill that is
/// turned off is left out, as Codex leaves it out of its own list.
#[must_use]
pub fn codex_skills(result: &Value) -> Option<DriveEvent> {
    let data = result.get("data")?.as_array()?;
    if data.is_empty() || !data.iter().all(|entry| entry["skills"].is_array()) {
        return None;
    }
    let skills = data
        .iter()
        .flat_map(|entry| list(&entry["skills"], MOST_ROWS))
        .filter(|skill| skill["enabled"].as_bool() != Some(false))
        .filter_map(codex_skill)
        .take(MOST_ROWS)
        .collect();
    Some(DriveEvent::Commands {
        lists: CommandLists {
            commands: Vec::new(),
            skills,
        },
    })
}

fn codex_skill(skill: &Value) -> Option<SkillRow> {
    let path = skill["path"].as_str()?;
    let fits = path.starts_with('/')
        && path.len() <= MOST_PATH
        && !path
            .chars()
            .any(|each| each.is_control() || is_hidden_mark(each));
    fits.then_some(())?;
    Some(SkillRow {
        name: key(&skill["name"], NAME_MORE)?,
        description: line(&skill["description"]),
        short: line(&skill["interface"]["shortDescription"])
            .or_else(|| line(&skill["shortDescription"])),
        scope: key(&skill["scope"], b"")?,
        path: path.to_owned(),
        user_only: skill["userInvocationOnly"].as_bool() == Some(true),
    })
}

/// A command of Claude Code from its file, before any thread runs in the folder: `name` is the
/// path of a file of `.claude/commands` with `:` for each folder and no `.md`, or the folder of
/// a `SKILL.md` of `.claude/skills`; `text` is the start of the file. The frontmatter gives the
/// name of a skill, the description and the hint; with no description, the first line of the
/// text is the description, as Claude Code shows it. `None` for a name that is not a plain key.
#[must_use]
pub fn claude_file(name: &str, text: &str) -> Option<CommandRow> {
    let (front, body) = frontmatter(text);
    let field = |wanted: &str| {
        front.iter().find_map(|(field, value)| {
            (*field == wanted).then(|| line(&Value::from(unquoted(value))))?
        })
    };
    let named = field("name").unwrap_or_else(|| name.to_owned());
    let description = field("description").or_else(|| {
        let first = body.lines().find(|each| !each.trim().is_empty())?;
        line(&Value::from(first.trim_start_matches('#').trim()))
    });
    Some(CommandRow {
        name: key(&Value::from(named), NAME_MORE)?,
        description,
        hint: field("argument-hint"),
        run: Run::Text,
        leaves_computer: false,
    })
}

/// The fields of a YAML frontmatter, one `field: value` on each line, and the text after it.
/// A value over more lines keeps only its first line.
fn frontmatter(text: &str) -> (Vec<(&str, &str)>, &str) {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (Vec::new(), text);
    };
    let Some(end) = rest.find("\n---") else {
        return (Vec::new(), text);
    };
    let fields = rest[..end]
        .lines()
        .filter_map(|each| each.split_once(':'))
        .map(|(field, value)| (field.trim(), value.trim()))
        .collect();
    let after = &rest[end + 4..];
    (fields, after.split_once('\n').map_or("", |(_, body)| body))
}

fn unquoted(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value)
}
