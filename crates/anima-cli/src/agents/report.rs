//! The usage table for a person and the notes below it (AIS-021, AIS-024).

use super::documents::agent_usage::{
    AgentSource, AgentUsageAgentUsage, AgentUsageAgentUsageAccounts, AgentUsageDimension,
    AgentUsageState, UsageSums,
};
use crate::output::{Cell, Table, Tone};

pub const SYSTEM_UNIT: &str =
    "This daemon runs as a system unit and cannot see the sessions of a user.";

pub const TURNED_OFF: &str = "The use counts are turned off on this device.";

#[must_use]
pub fn estimate_note(prices_as_of: &str) -> String {
    format!(
        "The cost is an estimate from the public list prices of {prices_as_of}; it is not the \
         bill of a subscription."
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Day,
    Device,
    Source,
    Model,
    Repository,
}

impl Column {
    #[must_use]
    pub const fn of(dimension: &AgentUsageDimension) -> Self {
        match dimension {
            AgentUsageDimension::DEVICE | AgentUsageDimension::Other(_) => Self::Device,
            AgentUsageDimension::SOURCE => Self::Source,
            AgentUsageDimension::MODEL => Self::Model,
            AgentUsageDimension::REPOSITORY => Self::Repository,
            AgentUsageDimension::DAY => Self::Day,
        }
    }

    const fn header(self) -> &'static str {
        match self {
            Self::Day => "DAY",
            Self::Device => "DEVICE",
            Self::Source => "SOURCE",
            Self::Model => "MODEL",
            Self::Repository => "REPOSITORY",
        }
    }

    fn cell(self, row: &UsageSums) -> String {
        let text = match self {
            Self::Day => row.day.clone(),
            Self::Device => row.device.as_ref().map(|device| device.name.clone()),
            Self::Source => row
                .source
                .as_ref()
                .map(|source| source_name(source).to_owned()),
            Self::Model => row.model.clone(),
            Self::Repository => row.repository.clone(),
        };
        text.unwrap_or_else(|| "-".to_owned())
    }
}

const COUNTS: [&str; 8] = [
    "SESSIONS",
    "PROMPTS",
    "TOOL CALLS",
    "INPUT",
    "OUTPUT",
    "CACHE READ",
    "CACHE WRITE",
    "EST. COST (USD)",
];

#[must_use]
pub fn table(answer: &AgentUsageAgentUsage, columns: &[Column]) -> Table {
    let mut header: Vec<&'static str> = columns.iter().map(|column| column.header()).collect();
    header.extend(COUNTS);
    let mut table = Table::new(header);
    for row in &answer.rows {
        let mut cells: Vec<Cell> = columns
            .iter()
            .map(|column| column.cell(row).into())
            .collect();
        cells.extend(sums(row));
        table.row(cells);
    }
    let mut total: Vec<Cell> = columns
        .iter()
        .enumerate()
        .map(|(at, _)| {
            if at == 0 {
                Cell::toned("total", Tone::Muted)
            } else {
                "".into()
            }
        })
        .collect();
    total.extend(sums(&answer.total));
    table.row(total);
    table
}

fn sums(row: &UsageSums) -> Vec<Cell> {
    let cost = row.estimated_cost_usd.map_or_else(
        || Cell::toned("no price", Tone::Muted),
        |usd| dollars(usd).into(),
    );
    vec![
        row.sessions.to_string().into(),
        whole(row.prompts).into(),
        whole(row.tool_calls).into(),
        tokens(row.input_tokens).into(),
        tokens(row.output_tokens).into(),
        tokens(row.cache_read_tokens).into(),
        tokens(row.cache_write_tokens).into(),
        cost,
    ]
}

#[must_use]
pub fn notes(answer: &AgentUsageAgentUsage) -> Vec<String> {
    let mut lines = vec![format!(
        "Days {} to {} (UTC).",
        answer.first_day, answer.last_day
    )];
    if answer.rows_left_out > 0 {
        lines.push(format!(
            "{} more rows are not shown: ask for fewer days or sum by fewer dimensions.",
            answer.rows_left_out
        ));
    }
    lines.push(estimate_note(&answer.prices_as_of));
    if answer.total.unpriced_tokens > 0.0 {
        lines.push(format!(
            "{} tokens of models with no known price are not in the estimate.",
            tokens(answer.total.unpriced_tokens)
        ));
    }
    for entry in &answer.devices {
        if let Some(sentence) = state_sentence(&entry.state) {
            lines.push(format!("{}: {sentence}", entry.device.name));
        }
    }
    lines.extend(answer.accounts.iter().map(account_line));
    lines
}

/// The limit of an account is shared by its devices (R-313).
#[must_use]
pub fn account_line(account: &AgentUsageAgentUsageAccounts) -> String {
    let plan = account
        .plan
        .as_deref()
        .map_or_else(String::new, |plan| format!(", {plan}"));
    let devices: Vec<&str> = account
        .devices
        .iter()
        .map(|one| one.name.as_str())
        .collect();
    let limits: Vec<String> = account
        .limits
        .iter()
        .map(|limit| {
            let used = match (limit.reached, limit.used_percent) {
                (true, _) => "reached".to_owned(),
                (false, Some(used)) => format!("{used} % used"),
                (false, None) => "not reached".to_owned(),
            };
            let resets = limit
                .resets_at
                .as_deref()
                .map_or_else(String::new, |at| format!(", resets {at}"));
            format!("{} {used}{resets}", limit.window)
        })
        .collect();
    let limits = if limits.is_empty() {
        "no limit reported".to_owned()
    } else {
        limits.join("; ")
    };
    format!(
        "Account {} ({}{plan}) on {}: {limits}.",
        account.short_key,
        source_name(&account.source),
        devices.join(", ")
    )
}

#[must_use]
pub const fn state_sentence(state: &AgentUsageState) -> Option<&'static str> {
    match state {
        AgentUsageState::SYSTEM_UNIT => Some(SYSTEM_UNIT),
        AgentUsageState::TURNED_OFF => Some(TURNED_OFF),
        AgentUsageState::READING | AgentUsageState::Other(_) => None,
    }
}

const fn source_name(source: &AgentSource) -> &'static str {
    match source {
        AgentSource::CLAUDE_CODE => "Claude Code",
        AgentSource::CODEX => "Codex",
        AgentSource::Other(_) => "other",
    }
}

fn whole(count: f64) -> String {
    format!("{count:.0}")
}

#[must_use]
pub fn tokens(count: f64) -> String {
    for (size, unit) in [(1e9, "B"), (1e6, "M"), (1e3, "k")] {
        if count >= size {
            return format!("{:.1}{unit}", count / size);
        }
    }
    whole(count)
}

fn dollars(usd: f64) -> String {
    if usd > 0.0 && usd < 0.005 {
        "<0.01".to_owned()
    } else {
        format!("{usd:.2}")
    }
}
