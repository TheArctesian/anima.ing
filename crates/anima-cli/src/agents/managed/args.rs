use crate::failure::Failure;
use agent_threads::{
    control::Action,
    drive::{Answer, Mode, Program},
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, clap::Args)]
pub struct Target {
    /// Enrolled device name or ID.
    pub device: String,
    pub workspace: Uuid,
    #[arg(long)]
    pub generation: u64,
    /// Reuse this ID only with the same command when retrying a lost submission.
    #[arg(long)]
    pub request: Option<Uuid>,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Provider {
    Codex,
    Claude,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Permission {
    FullAccess,
    AcceptEdits,
    Ask,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Decision {
    Allow,
    Deny,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Sign in to the provider in this workspace; closing the command cancels sign-in.
    Login {
        #[arg(long, value_enum, default_value = "codex")]
        provider: Provider,
        device: String,
        workspace: Uuid,
        #[arg(long)]
        generation: u64,
    },
    /// Prepare and manage an owned workspace on an enrolled node.
    Workspace {
        #[command(subcommand)]
        command: super::workspaces::Command,
    },
    /// Manage short-lived, private development-port grants. Open them in the Code app.
    Preview {
        #[command(subcommand)]
        command: super::previews::Command,
    },
    /// List workspace identities, generations, enablement and live readiness.
    Workspaces { device: String },
    /// Allow new Code commands in an offered workspace; --off revokes queued delivery.
    Enable {
        device: String,
        workspace: Uuid,
        #[arg(long)]
        off: bool,
    },
    /// Read a command receipt. Unresolved does not mean it failed to execute.
    Status { request: Uuid },
    /// Start Codex or Claude in the managed project directory.
    Start {
        #[command(flatten)]
        target: Target,
        #[arg(long, value_enum, default_value = "codex")]
        program: Provider,
        #[arg(long, value_enum, default_value = "full-access")]
        mode: Permission,
        #[arg(long)]
        model: Option<String>,
        /// UTF-8 prompt file, or - for standard input; never put secrets in shell history.
        #[arg(long)]
        prompt_file: PathBuf,
    },
    /// Send a prompt to the exact thread in this workspace generation.
    Send {
        #[command(flatten)]
        target: Target,
        thread: Uuid,
        #[arg(long)]
        prompt_file: PathBuf,
    },
    /// Answer the exact approval request displayed in the Code transcript.
    Answer {
        #[command(flatten)]
        target: Target,
        thread: Uuid,
        approval: Uuid,
        #[arg(value_enum)]
        decision: Decision,
    },
    /// Interrupt a turn while keeping its thread available.
    Interrupt {
        #[command(flatten)]
        target: Target,
        thread: Uuid,
    },
    /// End the thread's program and its children.
    Stop {
        #[command(flatten)]
        target: Target,
        thread: Uuid,
    },
}

pub(super) fn mutation(command: Command) -> Result<(Target, Action), Failure> {
    match command {
        Command::Start {
            target,
            program,
            mode,
            model,
            prompt_file,
        } => Ok((target, start(program, mode, model, prompt(&prompt_file)?))),
        Command::Send {
            target,
            thread,
            prompt_file,
        } => Ok((
            target,
            Action::Send {
                thread,
                text: prompt(&prompt_file)?,
                attachments: Vec::new(),
                skills: Vec::new(),
            },
        )),
        Command::Answer {
            target,
            thread,
            approval,
            decision,
        } => Ok((
            target,
            Action::Answer {
                thread,
                request: approval,
                answer: match decision {
                    Decision::Allow => Answer::Allow,
                    Decision::Deny => Answer::Deny {
                        message: "Denied by the person.".to_owned(),
                    },
                },
            },
        )),
        Command::Interrupt { target, thread } => Ok((target, Action::Interrupt { thread })),
        Command::Stop { target, thread } => Ok((target, Action::Stop { thread })),
        _ => Err(Failure::Usage {
            message: "Choose a managed Code mutation.".into(),
            hint: None,
        }),
    }
}

fn prompt(path: &std::path::Path) -> Result<String, Failure> {
    use std::io::Read;
    let failed = |_| Failure::Input {
        what: "prompt".into(),
        reason: "Expected at most 32 KiB of UTF-8 text.".into(),
    };
    let reader: Box<dyn Read> = if path == std::path::Path::new("-") {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).map_err(failed)?)
    };
    let mut bytes = Vec::new();
    reader.take(32769).read_to_end(&mut bytes).map_err(failed)?;
    if bytes.len() > 32768 {
        return Err(failed(std::io::Error::other("limit")));
    }
    String::from_utf8(bytes).map_err(|_| failed(std::io::Error::other("encoding")))
}

fn start(program: Provider, mode: Permission, model: Option<String>, prompt: String) -> Action {
    Action::Start {
        program: match program {
            Provider::Codex => Program::Codex,
            Provider::Claude => Program::ClaudeCode,
        },
        mode: match mode {
            Permission::FullAccess => Mode::FullAccess,
            Permission::AcceptEdits => Mode::AcceptEdits,
            Permission::Ask => Mode::Ask,
        },
        model,
        folder: "/home/agent/workspace".into(),
        prompt,
        provider_address: None,
        effort: None,
        speed: None,
        program_path: None,
        draft: None,
        attachments: Vec::new(),
        worktree: false,
        base: None,
    }
}
