mod shape;

use async_graphql::{Context, ErrorExtensions as _, ID, InputObject, Object, Result, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use self::shape::{
    AgentThreadAnswerPayload, AgentThreadCommandReceipt, AgentThreadControlView,
    AgentThreadLaunchProgram, AgentThreadLaunchReceipt, AgentThreadStopTargetInput,
};
use super::super::remote::{self, Error, Owner, Pair, Payload, Program, Start};
use crate::{
    api_errors::{internal, pool, schema_dependency},
    fleet::{access::person_only, events::ClientAddr},
};

#[derive(Default)]
pub struct RemoteQuery;

#[Object(name = "AgentThreadRemoteQuery")]
impl RemoteQuery {
    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn agent_thread_start_targets(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<AgentThreadStartTarget>> {
        let rows = remote::targets(pool(ctx)?, account(ctx)?)
            .await
            .map_err(|error| refusal(&error))?;
        Ok(rows
            .into_iter()
            .map(|row| AgentThreadStartTarget {
                device: row.device.to_string().into(),
                project: row.pair.project,
                folder: row.pair.folder,
                program: match row.pair.program {
                    Program::ClaudeCode => AgentThreadLaunchProgram::ClaudeCode,
                    Program::Codex => AgentThreadLaunchProgram::Codex,
                },
                expires_at: row.expires_at,
            })
            .collect())
    }

    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn agent_thread_launch(
        &self,
        ctx: &Context<'_>,
        launch: ID,
    ) -> Result<AgentThreadLaunchReceipt> {
        let row = remote::launch_receipt(pool(ctx)?, account(ctx)?, uuid(&launch)?)
            .await
            .map_err(|error| refusal(&error))?;
        shape::view(&row)
    }

    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn agent_thread_control(
        &self,
        ctx: &Context<'_>,
        thread: ID,
    ) -> Result<AgentThreadControlView> {
        let offer = remote::control(pool(ctx)?, account(ctx)?, uuid(&thread)?)
            .await
            .map_err(|error| refusal(&error))?;
        Ok(AgentThreadControlView {
            thread,
            offer: offer.as_ref().map(shape::view).transpose()?,
        })
    }

    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn agent_thread_command(
        &self,
        ctx: &Context<'_>,
        command: ID,
    ) -> Result<AgentThreadCommandReceipt> {
        let row = remote::command_receipt(pool(ctx)?, account(ctx)?, command.as_str())
            .await
            .map_err(|error| refusal(&error))?;
        shape::view(&row)
    }
}

#[derive(Default)]
pub struct RemoteMutation;

#[Object(name = "AgentThreadRemoteMutation")]
impl RemoteMutation {
    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn start_agent_thread(
        &self,
        ctx: &Context<'_>,
        input: AgentThreadStartInput,
    ) -> Result<AgentThreadLaunchReceipt> {
        let owner = Owner {
            account: account(ctx)?,
            device: uuid(&input.device)?,
        };
        let row = remote::start(
            pool(ctx)?,
            owner,
            Start {
                launch: uuid(&input.launch)?,
                pair: Pair {
                    project: input.project,
                    folder: input.folder,
                    program: match input.program {
                        AgentThreadLaunchProgram::ClaudeCode => Program::ClaudeCode,
                        AgentThreadLaunchProgram::Codex => Program::Codex,
                    },
                },
                access: input.access,
                prompt: input.prompt,
            },
            actor(ctx)?,
        )
        .await
        .map_err(|error| refusal(&error))?;
        shape::view(&row)
    }

    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn send_agent_thread_prompt(
        &self,
        ctx: &Context<'_>,
        input: AgentThreadPromptInput,
    ) -> Result<AgentThreadCommandReceipt> {
        command(
            ctx,
            (input.command, input.thread),
            Payload::Prompt {
                prompt: input.prompt,
            },
        )
        .await
    }

    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn answer_agent_thread_request(
        &self,
        ctx: &Context<'_>,
        input: AgentThreadAnswerInput,
    ) -> Result<AgentThreadCommandReceipt> {
        account(ctx)?;
        let payload = Payload::Answer {
            process: input.process.to_string(),
            revision: revision(input.revision)?,
            request: input.request.to_string(),
            answer: shape::input(&input.answer, "invalid_answer")?,
        };
        command(ctx, (input.command, input.thread), payload).await
    }

    #[graphql(guard = "crate::fleet::access::Authenticated")]
    async fn stop_agent_thread(
        &self,
        ctx: &Context<'_>,
        input: AgentThreadStopInput,
    ) -> Result<AgentThreadCommandReceipt> {
        account(ctx)?;
        let target = shape::input(&input.target, "target_stale")?;
        command(ctx, (input.command, input.thread), Payload::Stop { target }).await
    }
}

async fn command(
    ctx: &Context<'_>,
    (id, thread): (ID, ID),
    payload: Payload,
) -> Result<AgentThreadCommandReceipt> {
    let row = remote::admit(
        (pool(ctx)?, actor(ctx)?),
        account(ctx)?,
        (id.as_str(), uuid(&thread)?),
        payload,
    )
    .await
    .map_err(|error| refusal(&error))?;
    shape::view(&row)
}

fn actor(ctx: &Context<'_>) -> Result<remote::Actor> {
    let actor = person_only(ctx)?
        .operator()
        .ok_or_else(|| refused("FORBIDDEN"))?;
    Ok((actor.clone(), *schema_dependency::<ClientAddr>(ctx)?))
}

fn account(ctx: &Context<'_>) -> Result<Uuid> {
    let person = person_only(ctx)?
        .operator()
        .ok_or_else(|| refused("FORBIDDEN"))?;
    Ok(person.id.into())
}

fn uuid(id: &ID) -> Result<Uuid> {
    id.parse::<Uuid>()
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or_else(|| refused("MALFORMED"))
}

fn revision(value: f64) -> Result<i64> {
    if !(1.0..=9_007_199_254_740_991.0).contains(&value) || value.fract() != 0.0 {
        return Err(refused("invalid_answer"));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the range and fractional part were checked"
    )]
    let revision = value as i64;
    Ok(revision)
}

fn refusal(error: &Error) -> async_graphql::Error {
    match error {
        Error::Refused(code) => refused(code),
        // Errors at this boundary can originate from parsing or storing private content.
        _ => internal(std::io::Error::other("remote thread request failed")),
    }
}

fn refused(code: &'static str) -> async_graphql::Error {
    async_graphql::Error::new("Remote thread request refused.")
        .extend_with(|_, extensions| extensions.set("code", code))
}

#[derive(SimpleObject)]
struct AgentThreadStartTarget {
    device: ID,
    project: String,
    folder: String,
    program: AgentThreadLaunchProgram,
    expires_at: DateTime<Utc>,
}

#[derive(InputObject)]
struct AgentThreadStartInput {
    launch: ID,
    device: ID,
    project: String,
    folder: String,
    program: AgentThreadLaunchProgram,
    access: String,
    prompt: String,
}

#[derive(InputObject)]
struct AgentThreadPromptInput {
    command: ID,
    thread: ID,
    prompt: String,
}

#[derive(InputObject)]
struct AgentThreadAnswerInput {
    command: ID,
    thread: ID,
    process: ID,
    revision: f64,
    request: ID,
    answer: AgentThreadAnswerPayload,
}

#[derive(InputObject)]
struct AgentThreadStopInput {
    command: ID,
    thread: ID,
    target: AgentThreadStopTargetInput,
}
