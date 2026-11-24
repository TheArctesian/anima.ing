use graphql_client::GraphQLQuery;

use crate::{failure::Failure, hub::scalars::DateTime, invocation::Invocation, output::Table};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/auth/sessions.graphql",
    response_derives = "Serialize"
)]
struct CommandLineSessions;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/auth/sessions.graphql",
    response_derives = "Serialize"
)]
struct RevokeCommandLineSession;

pub(super) async fn list(invocation: &Invocation, offset: i64) -> Result<(), Failure> {
    let answer = invocation
        .hub()?
        .run::<CommandLineSessions>(command_line_sessions::Variables { offset })
        .await?;
    invocation
        .output
        .data(&answer.command_line_sessions, |sessions| {
            let mut table = Table::new(vec!["ID", "COMPUTER", "ACCESS", "LAST USED"]);
            for session in sessions {
                table.row(vec![
                    session.id.as_str().into(),
                    session.computer.as_str().into(),
                    (if session.linked {
                        "linked to app"
                    } else {
                        "until revoked"
                    })
                    .into(),
                    session.last_used_at.as_deref().unwrap_or("not yet").into(),
                ]);
            }
            table
        })
}

pub(super) async fn revoke(invocation: &Invocation, id: String) -> Result<(), Failure> {
    invocation.prompter.confirm(
        "Revoke this command-line session? Its device and hosted applications will keep running.",
    )?;
    let answer = invocation
        .hub()?
        .run::<RevokeCommandLineSession>(revoke_command_line_session::Variables { id })
        .await?;
    invocation.output.data(&answer, |answer| {
        let mut table = Table::record();
        table.pair(
            "Revoked",
            if answer.revoke_command_line_session {
                "yes"
            } else {
                "already absent"
            },
        );
        table
    })
}
