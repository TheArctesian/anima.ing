//! Keep a one-shot Docker usage sample separate from container configuration.
use super::docker;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_CPU_PERIOD_MICROS: u64 = 100_000;
const STATS: &str = concat!(
    r#"{"container":{{json .ID}},"cpu":{{json .CPUPerc}},"#,
    r#""memory":{{json .MemUsage}},"tasks":{{json .PIDs}}}"#,
);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Configuration {
    nano_cpus: Option<i64>,
    cpu_quota: Option<i64>,
    cpu_period: Option<i64>,
    cpu_set: Option<String>,
    memory_bytes: Option<i64>,
    tasks: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Limits<'a> {
    cpu_millis: Option<u64>,
    memory_bytes: Option<u64>,
    tasks: Option<u64>,
    cpu_set: Option<&'a str>,
}

impl Configuration {
    fn limits(&self) -> Limits<'_> {
        let cpu_millis = positive(self.nano_cpus)
            .map(|nano| nano / 1_000_000)
            .or_else(|| {
                let period = positive(self.cpu_period).unwrap_or(DEFAULT_CPU_PERIOD_MICROS);
                positive(self.cpu_quota).map(|quota| quota.saturating_mul(1000) / period)
            });
        Limits {
            cpu_millis,
            memory_bytes: positive(self.memory_bytes),
            tasks: positive(self.tasks),
            cpu_set: self.cpu_set.as_deref().filter(|set| !set.is_empty()),
        }
    }
}

fn positive(value: Option<i64>) -> Option<u64> {
    u64::try_from(value?).ok().filter(|value| *value > 0)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stats {
    container: String,
    cpu: String,
    memory: String,
    tasks: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    cpu_percent: f64,
    memory: String,
    tasks: u64,
}

async fn sample(container: &str) -> Option<Usage> {
    let output = docker(
        &[
            "stats",
            "--no-stream",
            "--no-trunc",
            "--format",
            STATS,
            container,
        ],
        4,
    )
    .await
    .ok()?;
    let stats: Stats = serde_json::from_str(&output).ok()?;
    if stats.container != container {
        return None;
    }
    let tasks = stats.tasks.parse::<u64>().ok().filter(|tasks| *tasks > 0)?;
    let cpu_percent = stats
        .cpu
        .strip_suffix('%')?
        .parse::<f64>()
        .ok()
        .filter(|cpu| cpu.is_finite() && *cpu >= 0.0)?;
    let (memory, _) = stats.memory.split_once(" / ")?;
    let unit = memory.find(|letter: char| letter.is_ascii_alphabetic())?;
    let (amount, unit) = memory.split_at(unit);
    Some(Usage {
        cpu_percent,
        memory: format!("{} {unit}", amount.trim_end()),
        tasks,
    })
}

pub(super) async fn observe(
    container: &str,
    state: &str,
    configured: Option<&Configuration>,
) -> Value {
    let usage = if matches!(state, "running" | "paused") {
        sample(container).await
    } else {
        None
    };
    json!({"limits":configured.map(Configuration::limits),"usage":usage})
}
