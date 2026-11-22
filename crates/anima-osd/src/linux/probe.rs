use serde::{Deserialize, Serialize};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

#[derive(Clone, Deserialize, Serialize)]
pub struct Diagnostics {
    pub service_root_read_only: Option<bool>,
    pub monitoring: UnitState,
    pub private_link: UnitState,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitState {
    Active,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Unknown,
}

pub async fn read(systemctl: &Path) -> Diagnostics {
    let (monitoring, private_link) = tokio::join!(
        unit(systemctl, "anima-daemon.service"),
        unit(systemctl, "anima-link.service")
    );
    Diagnostics {
        service_root_read_only: super::status::read().service_root_read_only,
        monitoring,
        private_link,
    }
}

async fn unit(program: &Path, name: &'static str) -> UnitState {
    let result = output(program, name).await;
    match result {
        Ok(value) => match value.trim() {
            "active" => UnitState::Active,
            "inactive" => UnitState::Inactive,
            "failed" => UnitState::Failed,
            "activating" => UnitState::Activating,
            "deactivating" => UnitState::Deactivating,
            _ => unavailable(name, "invalid_output"),
        },
        Err(category) => unavailable(name, category),
    }
}

fn unavailable(unit: &str, category: &str) -> UnitState {
    eprintln!("anima-osd: probe={unit} failure={category}");
    UnitState::Unknown
}

async fn output(program: &Path, name: &str) -> Result<String, &'static str> {
    let mut child = Command::new(program)
        .args(["show", "--property=ActiveState", "--value", "--", name])
        .env_clear()
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "spawn_failed")?;
    let result = timeout(Duration::from_secs(1), collect(&mut child))
        .await
        .unwrap_or(Err("timed_out"));
    if result.is_err() && child.try_wait().map_err(|_| "wait_failed")?.is_none() {
        child.kill().await.map_err(|_| "cleanup_failed")?;
    }
    result
}

async fn collect(child: &mut tokio::process::Child) -> Result<String, &'static str> {
    let stdout = child.stdout.take().ok_or("missing_output")?;
    let mut bytes = Vec::new();
    stdout
        .take(65)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "read_failed")?;
    if bytes.len() > 64 {
        return Err("output_limit");
    }
    if !child.wait().await.map_err(|_| "wait_failed")?.success() {
        return Err("exit_failed");
    }
    String::from_utf8(bytes).map_err(|_| "invalid_output")
}
