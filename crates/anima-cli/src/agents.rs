//! `anima agents`: the AI agent programs on the devices (R-135, AIS-020, AIS-025, CMD-030, THR-007).

#[cfg(unix)]
pub mod accounts;
pub mod documents;
#[cfg(unix)]
pub mod drive;
#[cfg(unix)]
pub mod files;
pub(crate) mod hub_threads;
#[cfg(unix)]
pub mod listing;
mod remote_threads;
pub mod report;
#[cfg(unix)]
pub mod runner;
pub mod sessions;
pub mod threads;
pub mod usage;
#[cfg(unix)]
pub mod watch_text;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum AgentCommand {
    /// Start, control and inspect threads on an enrolled device.
    Remote {
        #[command(subcommand)]
        command: remote_threads::RemoteCommand,
    },
    /// Sum the sessions, prompts, tool calls and tokens of Claude Code and Codex, with an
    /// estimate of their cost.
    Usage(usage::UsageArgs),
    /// List the single sessions of Claude Code and Codex that the daemons counted, the newest
    /// use first, with their sums and an estimate of their cost.
    Sessions(sessions::SessionsArgs),
    /// List the sessions of Claude Code and Codex on this computer as threads, the last
    /// changed first. Reads the files of this user; sends nothing to the hub.
    Threads(threads::ThreadsArgs),
    /// Show one thread of this computer: the prompts, the replies, the reasoning and the tool
    /// calls with their results.
    Thread(threads::ThreadArgs),
    /// Read a connected thread and its initial transcript.
    HubThread(hub_threads::ReadArgs),
    /// Stream a connected thread starting after an acknowledged sequence.
    WatchHubThread(hub_threads::WatchArgs),
    /// Start Claude Code or Codex in a folder of this computer as a new thread, through the
    /// thread runner of the daemon. Full access is the default: the program asks nothing.
    #[cfg(unix)]
    Start(drive::StartArgs),
    /// Send a prompt to a thread that this computer runs.
    #[cfg(unix)]
    Send(drive::SendArgs),
    /// Queue a prompt for a thread that this computer runs: it goes as the next turn when the
    /// turns before it ended well. Change, drop or send a queued prompt now.
    #[cfg(unix)]
    Queue(drive::QueueArgs),
    /// Change the model, the effort or the mode of a thread that this computer runs.
    #[cfg(unix)]
    Set(drive::SetArgs),
    /// Answer a request of a thread: allow (the default), deny, allow for the session, or
    /// choose the answers of a question.
    #[cfg(unix)]
    Approve(drive::ApproveArgs),
    /// Stop the running turn of a thread; the program stays.
    #[cfg(unix)]
    Interrupt(drive::ThreadIdArgs),
    /// Ask Codex to compact the context of a thread. For Claude Code, send the prompt /compact.
    #[cfg(unix)]
    Compact(drive::ThreadIdArgs),
    /// End the program of a thread and each process below it.
    #[cfg(unix)]
    Stop(drive::ThreadIdArgs),
    /// List the threads that the runner of this computer started, with their state.
    #[cfg(unix)]
    Running,
    /// Show the updates of the threads of this computer as they happen.
    #[cfg(unix)]
    Watch(drive::WatchArgs),
    /// Raise the soft limit of threads one step, after the question of the limit.
    #[cfg(unix)]
    RaiseLimit,
    /// Show the account, the plan and the limits that each agent program of this computer
    /// said. They stay on this computer.
    #[cfg(unix)]
    Accounts,
    /// Pin a thread of this computer: it is listed first. `--off` takes the pin away.
    #[cfg(unix)]
    Pin(listing::SwitchArgs),
    /// Archive a thread of this computer: it is listed only with the archived threads.
    #[cfg(unix)]
    Archive(listing::SwitchArgs),
    /// Give a thread of this computer a title of your own; with no title, the title of its
    /// program again.
    #[cfg(unix)]
    Rename(listing::RenameArgs),
    /// Remove a thread from the list of Anima, after a question. The session files of Claude
    /// Code or Codex stay on disk; a thread that runs is refused.
    #[cfg(unix)]
    Remove(listing::RemoveArgs),
    /// Settle a thread of this computer: it goes to the shelf Settled. `--off` takes it back.
    #[cfg(unix)]
    Settle(listing::SwitchArgs),
    /// Snooze a thread of this computer until a time (`--for 3h` or `--until <RFC 3339>`), at
    /// most 30 days. A prompt or a request of its program wakes it before.
    #[cfg(unix)]
    Snooze(listing::SnoozeArgs),
    /// End the snooze of a thread of this computer now.
    #[cfg(unix)]
    Wake(listing::WakeArgs),
    /// List the threads of this computer that are pinned, archived, renamed, removed, settled
    /// or snoozed.
    #[cfg(unix)]
    Listings,
    /// Show the models that each agent program of this computer offers for a start.
    #[cfg(unix)]
    Models,
    /// List the projects of this computer, or add, rename or remove one. A project is a folder
    /// whose threads are listed under its name.
    #[cfg(unix)]
    Projects(listing::ProjectsArgs),
    /// List the files and folders of the working folder of a thread that the runner started,
    /// the best match for the words first, as the `@` list of the composer. Secrets and the
    /// folders of tools and of build output are left out.
    #[cfg(unix)]
    Files(files::FilesArgs),
    /// List the commands and the skills of an agent program, as the / and $ lists of the
    /// composer: of a thread that the runner started, or of a program in a folder.
    #[cfg(unix)]
    Commands(files::CommandsArgs),
    /// List the folders in one folder of this computer, to choose a project for a thread. Only
    /// names of folders: no file and no link.
    #[cfg(unix)]
    Folders(files::FoldersArgs),
}

impl AgentCommand {
    /// Runs the command.
    ///
    /// # Errors
    /// [`Failure`] for a refusal, an unknown device or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Remote { command } => command.run(invocation).await,
            Self::Usage(args) => args.run(invocation).await,
            Self::Sessions(args) => args.run(invocation).await,
            Self::Threads(args) => args.run(invocation).await,
            Self::Thread(args) => args.run(invocation),
            Self::HubThread(args) => args.run(invocation).await,
            Self::WatchHubThread(args) => args.run(invocation).await,
            #[cfg(unix)]
            local => local.run_local(invocation).await,
        }
    }

    /// Runs a command that drives the runner of this computer.
    #[cfg(unix)]
    async fn run_local(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Start(args) => args.run(invocation).await,
            Self::Send(args) => args.run(invocation).await,
            Self::Queue(args) => args.run(invocation).await,
            Self::Set(args) => args.run(invocation).await,
            Self::Approve(args) => args.run(invocation).await,
            Self::Interrupt(args) => drive::interrupt(args, invocation).await,
            Self::Compact(args) => drive::compact(args, invocation).await,
            Self::Stop(args) => drive::stop(args, invocation).await,
            Self::Watch(args) => drive::watch(args, invocation).await,
            Self::RaiseLimit => drive::raise_limit(invocation).await,
            other => other.run_read(invocation).await,
        }
    }

    /// Runs a command that only reads what the runner holds.
    #[cfg(unix)]
    async fn run_read(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Running => drive::running(invocation).await,
            Self::Accounts => accounts::accounts(invocation).await,
            Self::Models => accounts::models(invocation).await,
            Self::Files(args) => files::files(args, invocation).await,
            Self::Commands(args) => files::commands(args, invocation).await,
            Self::Folders(args) => files::folders(args, invocation).await,
            other => other.run_listing(invocation).await,
        }
    }

    /// Runs a command that changes how a thread is listed (TH46, TH61).
    #[cfg(unix)]
    async fn run_listing(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Pin(args) => listing::pin(args, invocation).await,
            Self::Archive(args) => listing::archive(args, invocation).await,
            Self::Rename(args) => listing::rename(args, invocation).await,
            Self::Remove(args) => listing::remove(args, invocation).await,
            Self::Settle(args) => listing::settle(args, invocation).await,
            Self::Snooze(args) => listing::snooze(args, invocation).await,
            Self::Wake(args) => listing::wake(args, invocation).await,
            Self::Listings => listing::listings(invocation).await,
            Self::Projects(args) => listing::projects(args, invocation).await,
            _ => unreachable!("the other commands are run by `run`, `run_local` and `run_read`"),
        }
    }
}
