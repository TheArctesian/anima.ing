//! SHA-015, SHA-017, SHA-018.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::Serialize;
use serde_json::Value;

use super::measure::{self, BYTES_FOR_A_TOKEN, Sizes};
use super::setting::{self, State};
use super::store::{OriginalError, Store};
use super::{FOLDER, originals_dir, worth_it};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::Table;

#[derive(Debug, clap::Subcommand)]
pub enum ShorteningCommand {
    /// Show if the MCP server shortens long answers, and what the store holds.
    Show,
    /// Shorten long answers of the MCP server: rows that say the same are left out, never a
    /// row that reports a fault. The server reads this at its start.
    On,
    /// Give each answer whole.
    Off,
    /// Print, for each tool, the size of its shortened answers before and after.
    Measure,
    /// Print the whole answer behind a handle.
    Original {
        /// The handle from the field `shortened` of an answer.
        handle: String,
    },
    /// List the stored originals, the oldest first.
    Originals,
    /// Remove each stored original.
    Clear,
    /// Shorten the answer in a JSON file and print the sizes; nothing is stored or sent.
    Try {
        /// A file with one JSON object: the answer of a tool.
        file: PathBuf,
        /// Print the shortened answer in place of the sizes.
        #[arg(long)]
        answer: bool,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Shown {
    state: State,
    folder: PathBuf,
    originals: usize,
    bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Measured {
    tool: String,
    #[serde(flatten)]
    sizes: Sizes,
    tokens_before: u64,
    tokens_after: u64,
    saved_percent: u64,
}

impl Measured {
    fn of(tool: &str, sizes: Sizes) -> Self {
        Self {
            tool: tool.to_owned(),
            sizes,
            tokens_before: sizes.tokens_before(),
            tokens_after: sizes.tokens_after(),
            saved_percent: sizes.saved_percent(),
        }
    }
}

const ESTIMATE: &str =
    "Tokens are an estimate: the bytes of JSON divided by 4. No tokenizer of a model is used.";

impl ShorteningCommand {
    /// Runs the command on the folder of this user.
    ///
    /// # Errors
    /// [`Failure`] for a missing configuration folder, a handle that is gone, or a file that
    /// cannot be read or written.
    pub fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let dir = invocation.config_dir()?.join(FOLDER);
        match self {
            Self::Show => show(invocation),
            Self::On | Self::Off => {
                let state = if matches!(self, Self::On) {
                    State::On
                } else {
                    State::Off
                };
                setting::write(&dir, state).map_err(|error| written(&error))?;
                show(invocation)
            }
            Self::Measure => {
                let rows: Vec<Measured> = measure::read(&dir)
                    .iter()
                    .map(|(tool, sizes)| Measured::of(tool, *sizes))
                    .collect();
                invocation.output.data(&rows, |rows| measure_table(rows))?;
                invocation.output.note(ESTIMATE);
                Ok(())
            }
            Self::Original { handle } => {
                let original = store(invocation)?
                    .get(&handle, SystemTime::now())
                    .map_err(|error| lost(&error))?;
                invocation.output.raw(&original.answer)
            }
            Self::Originals => {
                let kept = store(invocation)?.list().map_err(|error| lost(&error))?;
                let rows: Vec<(String, u64, u64)> = kept
                    .iter()
                    .map(|each| (each.handle.clone(), each.bytes, age_minutes(each.written)))
                    .collect();
                invocation.output.data(&rows, |rows| originals_table(rows))
            }
            Self::Clear => {
                let removed = store(invocation)?.clear().map_err(|error| lost(&error))?;
                let note = format!("Removed {removed} originals of the selected hub.");
                invocation.output.note(&note);
                Ok(())
            }
            Self::Try { file, answer } => try_file(invocation, &file, answer),
        }
    }
}

/// The originals of the selected hub.
fn store(invocation: &Invocation) -> Result<Store, Failure> {
    let origin = invocation.origin()?.as_text();
    Ok(Store::new(originals_dir(invocation.config_dir()?, &origin)))
}

/// The state, and what the store of the selected hub holds when a hub is selected.
fn show(invocation: &Invocation) -> Result<(), Failure> {
    let kept = match store(invocation) {
        Ok(store) => store.list().map_err(|error| lost(&error))?,
        Err(_) => Vec::new(),
    };
    let folder = invocation.config_dir()?.join(FOLDER);
    let shown = Shown {
        state: setting::read(&folder),
        folder,
        originals: kept.len(),
        bytes: kept.iter().map(|each| each.bytes).sum(),
    };
    invocation.output.data(&shown, |shown| {
        let folder = shown.folder.display().to_string();
        let mut table = Table::default();
        table.row(vec!["Shortening".into(), shown.state.word().into()]);
        table.row(vec!["Folder".into(), folder.as_str().into()]);
        table.row(vec![
            "Originals of this hub".into(),
            shown.originals.to_string().as_str().into(),
        ]);
        table.row(vec![
            "Bytes".into(),
            shown.bytes.to_string().as_str().into(),
        ]);
        table
    })?;
    invocation.output.note(
        "A running `anima mcp` keeps the state it started with. Originals stay on this \
         computer, for each hub at most 24 hours and 64 MB.",
    );
    Ok(())
}

fn measure_table(rows: &[Measured]) -> Table {
    let mut table = Table::new(vec![
        "TOOL",
        "ANSWERS",
        "BYTES BEFORE",
        "BYTES AFTER",
        "TOKENS BEFORE",
        "TOKENS AFTER",
        "SAVED",
    ]);
    for row in rows {
        table.row(vec![
            row.tool.as_str().into(),
            row.sizes.answers.to_string().as_str().into(),
            row.sizes.bytes_before.to_string().as_str().into(),
            row.sizes.bytes_after.to_string().as_str().into(),
            row.tokens_before.to_string().as_str().into(),
            row.tokens_after.to_string().as_str().into(),
            format!("{} %", row.saved_percent).as_str().into(),
        ]);
    }
    table
}

fn originals_table(rows: &[(String, u64, u64)]) -> Table {
    let mut table = Table::new(vec!["HANDLE", "BYTES", "AGE (MINUTES)"]);
    for (handle, bytes, age) in rows {
        table.row(vec![
            handle.as_str().into(),
            bytes.to_string().as_str().into(),
            age.to_string().as_str().into(),
        ]);
    }
    table
}

fn age_minutes(written: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(written)
        .map_or(0, |age| age.as_secs() / 60)
}

/// Shortens the answer in `file` with the handle of its text, and prints the sizes or the
/// answer. It writes nothing.
fn try_file(invocation: &Invocation, file: &std::path::Path, answer: bool) -> Result<(), Failure> {
    let input = |reason: String| Failure::Input {
        what: file.display().to_string(),
        reason,
    };
    let text = std::fs::read_to_string(file).map_err(|error| input(error.to_string()))?;
    let whole: Value = serde_json::from_str(&text).map_err(|error| input(error.to_string()))?;
    let worth = worth_it("try", &whole);
    if answer {
        let shown = worth.as_ref().map_or(&whole, |worth| &worth.marked);
        return invocation.output.raw(shown);
    }
    let before = whole.to_string().len();
    let mut sizes = Sizes::default();
    sizes.add(before, worth.as_ref().map_or(before, |worth| worth.after));
    let name = if worth.is_some() {
        "shortened"
    } else {
        "not shortened"
    };
    let row = Measured::of(name, sizes);
    invocation.output.data(&[row], |rows| measure_table(rows))?;
    invocation.output.note(&format!(
        "{ESTIMATE} ({BYTES_FOR_A_TOKEN} bytes for a token.)"
    ));
    Ok(())
}

fn lost(error: &OriginalError) -> Failure {
    match error {
        OriginalError::NotAHandle => Failure::Usage {
            message: error.to_string(),
            hint: None,
        },
        OriginalError::Gone => Failure::NotFound {
            message: error.to_string(),
            hint: None,
        },
        _ => Failure::Write {
            reason: error.to_string(),
        },
    }
}

fn written(error: &crate::private_file::PrivateFileError) -> Failure {
    Failure::Write {
        reason: format!("the setting of the shortening was not saved ({error})"),
    }
}
