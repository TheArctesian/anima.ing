//! `anima model`: the model library of the hub (LIBM-001 to LIBM-008, SEL-003, SEL-005), and
//! the copies on the machines (`crate::model_copies`).

pub mod add;
pub mod alias;
pub mod documents;
pub mod list;
pub mod show;
pub mod source;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum ModelCommand {
    /// List the models of the library with their variants.
    List(list::ListArgs),
    /// Show one model by its name, alias or name:tag, with its files and SHA-256 values.
    Show(show::ShowArgs),
    /// Read a Hugging Face repository of an approved organisation into the library.
    Add(add::AddArgs),
    /// Point an alias, such as coder, at a model.
    Alias(alias::AliasArgs),
    /// The approved organisations of the sources (list, add, mode).
    #[command(subcommand)]
    Source(source::SourceCommand),
    /// Pull a variant name:tag to a machine: it downloads and verifies the files.
    Pull(crate::model_copies::PullArgs),
    /// Remove the copy of a variant from a machine.
    Remove(crate::model_copies::RemoveArgs),
    /// List the copies on the machines, with their state and the progress of a pull.
    Copies(crate::model_copies::CopiesArgs),
    /// List the models that the machines found where Ollama, LM Studio or Hugging Face keeps them.
    Found(crate::model_copies::FoundArgs),
    /// Ask a machine to read its found models again.
    Find(crate::model_copies::FindArgs),
    /// Use a found model where it is: the machine checks its files at each load.
    Use(crate::model_copies::UseArgs),
    /// Pin a copy: its machine keeps it loaded, and loads it again when it gets the memory back.
    Pin(crate::model_copies::KeepArgs),
    /// Unpin a copy: its machine unloads it after its idle seconds with no use.
    Unpin(crate::model_copies::KeepArgs),
}

impl ModelCommand {
    /// Runs the command.
    ///
    /// # Errors
    /// [`Failure`] for a refusal, an unknown model, or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::List(args) => args.run(invocation).await,
            Self::Show(args) => args.run(invocation).await,
            Self::Add(args) => args.run(invocation).await,
            Self::Alias(args) => args.run(invocation).await,
            Self::Source(command) => command.run(invocation).await,
            Self::Pull(args) => args.run(invocation).await,
            Self::Remove(args) => args.run(invocation).await,
            Self::Copies(args) => args.run(invocation).await,
            Self::Found(args) => args.run(invocation).await,
            Self::Find(args) => args.run(invocation).await,
            Self::Use(args) => args.run(invocation).await,
            Self::Pin(args) => args.run(invocation, true).await,
            Self::Unpin(args) => args.run(invocation, false).await,
        }
    }
}

/// A size in bytes as GiB with one decimal, for a table.
#[must_use]
pub fn gib(bytes: f64) -> String {
    format!("{:.1} GiB", bytes / 1_073_741_824.0)
}
