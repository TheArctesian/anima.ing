//! `anima agents usage` (AIS-020, AIS-021, AIS-024).

use chrono::{Days, NaiveDate, Utc};

use super::documents::{AgentUsage, agent_usage};
use super::report;
use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::hub::Hub;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct UsageArgs {
    /// Only this device: its ID or its exact name.
    #[arg(long)]
    device: Option<String>,
    /// Only this program.
    #[arg(long, value_enum)]
    source: Option<SourceArg>,
    /// Only this repository, as host/owner/name (github.com/owner/name).
    #[arg(long)]
    repository: Option<String>,
    /// The first UTC day, YYYY-MM-DD; 29 days before --until when not given.
    #[arg(long, conflicts_with = "days")]
    since: Option<String>,
    /// The last UTC day, YYYY-MM-DD, included; today when not given.
    #[arg(long)]
    until: Option<String>,
    /// The number of days that end on --until (or today), 1 to 366.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..=366))]
    days: Option<u16>,
    /// Sum by these, repeated or separated by commas; device and source when not given.
    #[arg(long, value_enum, value_delimiter = ',')]
    by: Vec<DimensionArg>,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum SourceArg {
    ClaudeCode,
    Codex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum DimensionArg {
    Device,
    Source,
    Model,
    Repository,
    Day,
}

impl DimensionArg {
    const fn k1(self) -> agent_usage::AgentUsageDimension {
        use agent_usage::AgentUsageDimension as K1;
        match self {
            Self::Device => K1::DEVICE,
            Self::Source => K1::SOURCE,
            Self::Model => K1::MODEL,
            Self::Repository => K1::REPOSITORY,
            Self::Day => K1::DAY,
        }
    }
}

#[derive(Debug, Default)]
pub struct UsageAsk {
    pub device: Option<String>,
    pub source: Option<agent_usage::AgentSource>,
    pub repository: Option<String>,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub by: Vec<agent_usage::AgentUsageDimension>,
}

/// # Errors
/// [`Failure::NotFound`] for an unknown device name; the hub's refusal.
pub async fn fetch(hub: &Hub, ask: UsageAsk) -> Result<agent_usage::AgentUsageAgentUsage, Failure> {
    let device_id = match ask.device {
        Some(device) => Some(resolve(hub, &device).await?),
        None => None,
    };
    let filter = agent_usage::AgentUsageFilter {
        device_id,
        source: ask.source,
        repository: ask.repository,
        first_day: ask.first_day,
        last_day: ask.last_day,
    };
    let variables = agent_usage::Variables {
        filter: Some(filter),
        by: ask.by,
    };
    Ok(hub.run::<AgentUsage>(variables).await?.agent_usage)
}

impl UsageArgs {
    /// # Errors
    /// [`Failure::Usage`] for a bad `--until` with `--days`; as [`fetch`].
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let (first_day, last_day) =
            day_range((self.since, self.until), self.days, Utc::now().date_naive())?;
        let mut by: Vec<DimensionArg> = Vec::new();
        for one in self.by {
            if !by.contains(&one) {
                by.push(one);
            }
        }
        if by.is_empty() {
            by = vec![DimensionArg::Device, DimensionArg::Source];
        }
        let ask = UsageAsk {
            device: self.device,
            source: self.source.map(|source| match source {
                SourceArg::ClaudeCode => agent_usage::AgentSource::CLAUDE_CODE,
                SourceArg::Codex => agent_usage::AgentSource::CODEX,
            }),
            repository: self.repository,
            first_day,
            last_day,
            by: by.iter().map(|one| one.k1()).collect(),
        };
        let answer = fetch(&hub, ask).await?;
        let shown: Vec<report::Column> =
            by.iter().map(|one| report::Column::of(&one.k1())).collect();
        invocation
            .output
            .data(&answer, |answer| report::table(answer, &shown))?;
        if !invocation.output.json() {
            for line in report::notes(&answer) {
                invocation.output.line(&line)?;
            }
        }
        Ok(())
    }
}

pub(super) fn day_range(
    (since, until): (Option<String>, Option<String>),
    days: Option<u16>,
    today: NaiveDate,
) -> Result<(Option<String>, Option<String>), Failure> {
    let Some(days) = days else {
        return Ok((since, until));
    };
    let last = match until {
        Some(text) => NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(|_| Failure::Usage {
            message: format!("--until {text} is not a day: give it as YYYY-MM-DD."),
            hint: None,
        })?,
        None => today,
    };
    let first = last - Days::new(u64::from(days) - 1);
    Ok((Some(first.to_string()), Some(last.to_string())))
}
