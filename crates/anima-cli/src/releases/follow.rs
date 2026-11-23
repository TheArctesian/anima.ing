//! `anima release log --follow` (K1 `buildJobUpdated`, BLD-081).

use super::documents::{BuildJobUpdated, build_job_updated};
use crate::failure::Failure;
use crate::hub::subscription::Subscription;
use crate::invocation::Invocation;
use crate::output::stream::{self, Reader};
use crate::output::text::enum_name;

type State = build_job_updated::BuildJobState;

/// With `--json`, prints each K1 `BuildJobUpdate` as one object per line.
///
/// # Errors
/// [`Failure`] when the hub refuses the token or the connection is lost.
pub async fn run(job: &str, invocation: &Invocation) -> Result<(), Failure> {
    let hub = invocation.hub()?;
    let variables = build_job_updated::Variables {
        job_id: Some(job.to_owned()),
    };
    let mut updates = Subscription::open::<BuildJobUpdated>(&hub, variables).await?;
    while let Some(answer) = updates.next().await {
        let update = answer?.build_job_updated;
        if print(&update, invocation.output.json())? == Reader::Gone {
            break;
        }
        if let Some(end) = ending(&update.job) {
            invocation.output.note(&end);
            break;
        }
    }
    Ok(())
}

fn print(
    update: &build_job_updated::BuildJobUpdatedBuildJobUpdated,
    json: bool,
) -> Result<Reader, Failure> {
    if json {
        return stream::json_line(update);
    }
    for line in &update.log_lines {
        if stream::line(&line.text)? == Reader::Gone {
            return Ok(Reader::Gone);
        }
    }
    Ok(Reader::Open)
}

fn ending(job: &build_job_updated::BuildJobUpdatedBuildJobUpdatedJob) -> Option<String> {
    if !matches!(
        job.state,
        State::PUBLISHED | State::FAILED | State::CANCELLED
    ) {
        return None;
    }
    let reason = job
        .failure_reason
        .as_deref()
        .map_or_default(|reason| format!(": {reason}"));
    Some(format!("The job is {}{reason}.", enum_name(&job.state)))
}
