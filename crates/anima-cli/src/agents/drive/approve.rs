//! `anima agents approve`: the person answers a request of a thread (TH13): allow, allow for the
//! session, deny with a message, or the choices of a question.

use agent_threads::control::Action;
use agent_threads::drive::Answer;
use uuid::Uuid;

use super::simple;
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct ApproveArgs {
    thread: Uuid,
    /// The ID of the request, as `anima agents watch` shows it.
    request: Uuid,
    /// Deny, with this message to the agent.
    #[arg(long, conflicts_with_all = ["for_session", "choose"])]
    deny: Option<String>,
    /// Allow, and send back the session rule at this place of the request (0 is the first).
    #[arg(long, conflicts_with = "choose")]
    for_session: Option<usize>,
    /// The answer to a question, as KEY=CHOICE; once for each choice.
    #[arg(long, value_parser = parse_choice)]
    choose: Vec<(String, String)>,
}

/// One answer of `--choose`: KEY=CHOICE.
fn parse_choice(text: &str) -> Result<(String, String), String> {
    text.split_once('=')
        .map(|(key, choice)| (key.to_owned(), choice.to_owned()))
        .ok_or_else(|| "KEY=CHOICE".to_owned())
}

impl ApproveArgs {
    /// # Errors
    /// [`Failure`] for a refusal of the runner.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let answer = match (self.deny, self.for_session, self.choose.is_empty()) {
            (Some(message), _, _) => Answer::Deny { message },
            (None, Some(rule), _) => Answer::AllowForSession { rule },
            (None, None, false) => Answer::Choose {
                answers: self.choose,
            },
            (None, None, true) => Answer::Allow,
        };
        let action = Action::Answer {
            thread: self.thread,
            request: self.request,
            answer,
        };
        simple(invocation, action, "Answered.").await
    }
}
