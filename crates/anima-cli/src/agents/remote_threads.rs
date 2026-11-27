//! Person-only remote operations. Mutation input comes from stdin, outside shell arguments.

use std::io::Read as _;

use graphql_client::GraphQLQuery;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    failure::Failure,
    hub::{scalars::DateTime, token::TokenKind},
    invocation::Invocation,
};

macro_rules! operations {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "schema.graphql",
            query_path = "src/agents/remote_threads.graphql",
            response_derives = "Serialize",
            variables_derives = "Deserialize"
        )]
        struct $name;
    )+};
}

operations!(
    RemoteTargets,
    RemoteLaunchResult,
    RemoteCommandResult,
    RemoteControl,
    RemoteStart,
    RemotePrompt,
    RemoteAnswer,
    RemoteStop,
);

#[derive(Debug, clap::Subcommand)]
pub enum RemoteCommand {
    /// Show currently advertised device, project, folder and program pairs.
    Targets,
    /// Look up a launch and its first command ID.
    LaunchResult { launch: String },
    /// Look up the receipt of a delivered or pending command.
    CommandResult { command: String },
    /// Read the current control offer of a connected thread.
    Control { thread: String },
    /// Start a thread and reserve its first prompt. Read `AgentThreadStartInput` JSON from stdin.
    Start,
    /// Send a prompt. Read `AgentThreadPromptInput` JSON from stdin.
    Prompt,
    /// Answer the exact live request. Read `AgentThreadAnswerInput` JSON from stdin.
    Answer,
    /// Stop a turn or end a process. Read `AgentThreadStopInput` JSON from stdin.
    Stop,
}

impl RemoteCommand {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = person_hub(invocation)?;
        match self {
            Self::Targets => print(
                invocation,
                &hub.run::<RemoteTargets>(remote_targets::Variables)
                    .await?
                    .agent_thread_start_targets,
            ),
            Self::LaunchResult { launch } => print(
                invocation,
                &hub.run::<RemoteLaunchResult>(remote_launch_result::Variables { launch })
                    .await?
                    .agent_thread_launch,
            ),
            Self::CommandResult { command } => print(
                invocation,
                &hub.run::<RemoteCommandResult>(remote_command_result::Variables { command })
                    .await?
                    .agent_thread_command,
            ),
            Self::Control { thread } => print(
                invocation,
                &hub.run::<RemoteControl>(remote_control::Variables { thread })
                    .await?
                    .agent_thread_control,
            ),
            Self::Start => print(
                invocation,
                &hub.run::<RemoteStart>(remote_start::Variables { input: read()? })
                    .await?
                    .start_agent_thread,
            ),
            Self::Prompt => print(
                invocation,
                &hub.run::<RemotePrompt>(remote_prompt::Variables { input: read()? })
                    .await?
                    .send_agent_thread_prompt,
            ),
            Self::Answer => print(
                invocation,
                &hub.run::<RemoteAnswer>(remote_answer::Variables { input: read()? })
                    .await?
                    .answer_agent_thread_request,
            ),
            Self::Stop => print(
                invocation,
                &hub.run::<RemoteStop>(remote_stop::Variables { input: read()? })
                    .await?
                    .stop_agent_thread,
            ),
        }
    }
}

fn person_hub(invocation: &Invocation) -> Result<crate::hub::Hub, Failure> {
    let hub = invocation.hub()?;
    if hub
        .token()
        .is_some_and(|token| token.kind() == TokenKind::ApiKey)
    {
        return Err(Failure::Forbidden {
            message: "Remote thread operations require a person login.".to_owned(),
            hint: None,
        });
    }
    Ok(hub)
}

fn read<T: DeserializeOwned>() -> Result<T, Failure> {
    const LIMIT: u64 = 512 * 1024;
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    let refused = || Failure::Input {
        what: "remote thread input".to_owned(),
        reason: "expected one bounded JSON object on standard input".to_owned(),
    };
    let length = std::io::stdin()
        .lock()
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| refused())?;
    if u64::try_from(length).map_or(true, |length| length > LIMIT) {
        return Err(refused());
    }
    serde_json::from_slice(&bytes).map_err(|_| refused())
}

fn print(invocation: &Invocation, value: &impl Serialize) -> Result<(), Failure> {
    let value = serde_json::to_value(value).map_err(|_| Failure::Write {
        reason: "Could not encode the remote thread receipt.".to_owned(),
    })?;
    invocation.output.raw(&value)
}
