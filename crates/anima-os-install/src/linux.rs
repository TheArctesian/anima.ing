mod command;
mod disks;
mod install;
mod persist;
mod volumes;

use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{
    io::Read as _,
    path::{Path, PathBuf},
    process::ExitCode,
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error(
        "The installer could not access local storage. Stop and inspect the device before retrying."
    )]
    Io(#[from] std::io::Error),
    #[error("The installation request or disk inventory is invalid.")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Storage(#[from] anima_os_storage::Error),
    #[error("The installation command {0} failed. Check the local system journal.")]
    Command(&'static str),
    #[error(
        "The installation command {0} timed out. Check the installation media and target disk before retrying."
    )]
    CommandTimeout(&'static str),
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Parser)]
#[command(about = "Offline Anima OS disk installation; normal setup uses the graphical installer")]
struct Arguments {
    #[command(subcommand)]
    command: Operation,
}
#[derive(Subcommand)]
enum Operation {
    Inventory,
    Install {
        #[arg(long)]
        system: PathBuf,
    },
    #[command(hide = true)]
    MountVolumes,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    system: PathBuf,
    selected: disks::Disk,
    #[serde(default)]
    data: Vec<disks::Disk>,
    confirmed: Vec<disks::Identity>,
    hashed_password: String,
    keymap: String,
}

impl Drop for Plan {
    fn drop(&mut self) {
        self.hashed_password.zeroize();
    }
}

impl Plan {
    fn read(expected: &Path) -> Result<Self> {
        let mut bytes = Zeroizing::new(Vec::new());
        std::io::stdin()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(Error::Refused("The installation request is too large."));
        }
        let plan: Self = serde_json::from_slice(&bytes)?;
        if plan.system != expected
            || plan.data.len() > anima_os_storage::MAX_VOLUMES
            || plan.keymap.is_empty()
            || plan.keymap.len() > 1024 * 1024
            || plan.keymap.contains('\0')
            || !plan.hashed_password.starts_with("$y$")
            || plan.hashed_password.len() > 4096
            || plan
                .hashed_password
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && !b"$./".contains(&byte))
        {
            return Err(Error::Refused(
                "The confirmed installation settings are invalid; no disk changed.",
            ));
        }
        let identities: Vec<_> = std::iter::once(&plan.selected)
            .chain(&plan.data)
            .map(disks::Disk::identity)
            .collect();
        if identities != plan.confirmed {
            return Err(Error::Refused(
                "Disk authorization does not match the selected identities; review again.",
            ));
        }
        Ok(plan)
    }
}

pub fn main() -> ExitCode {
    let args = Arguments::parse();
    let result = match args.command {
        Operation::Inventory => disks::inventory().and_then(|disks| {
            let values: Vec<_> = disks
                .into_iter()
                .map(|mut disk| {
                    disk.eligible = Some(disk.allowed());
                    disk
                })
                .collect();
            println!("{}", serde_json::to_string(&values)?);
            Ok(())
        }),
        Operation::Install { system } => Plan::read(&system).and_then(|plan| install::run(&plan)),
        Operation::MountVolumes => anima_os_storage::mount_recorded().map_err(Error::from),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"state":"failed", "error":error.to_string()})
            );
            ExitCode::FAILURE
        }
    }
}

fn progress(phase: &str, progress: f64) {
    use std::io::Write as _;
    println!(
        "{}",
        serde_json::json!({"state":"running", "phase":phase, "progress":progress})
    );
    let _ = std::io::stdout().flush();
}
