//! Query only the checked local Docker engine and the installed Compose binary.
use std::time::{Duration, Instant};

use super::Error;
use crate::{
    Command, Paths,
    execution::{Executor, process},
    start::Version,
};

pub(super) fn probe(paths: &Paths) -> Result<(Version, Version), Error> {
    // The caller has already verified the paths and account. Open the executor to
    // check the store and the engine socket again, and hold the owner lock during probing.
    let _executor = Executor::open(paths)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let docker = Command {
        program: paths.docker.clone(),
        arguments: vec![
            "--host".into(),
            format!("unix://{}", paths.socket),
            "--config".into(),
            paths.configuration.clone(),
            "version".into(),
            "--format".into(),
            "{{.Server.Version}}".into(),
        ],
        environment: paths.environment(),
        folder: None,
    };
    let engine = version(&process::capture(paths, &docker, deadline)?)?;
    let compose = Command {
        program: paths.compose.clone(),
        arguments: vec!["version".into(), "--short".into()],
        environment: paths.environment(),
        folder: None,
    };
    Ok((
        engine,
        version(&process::capture(paths, &compose, deadline)?)?,
    ))
}

fn version(output: &[u8]) -> Result<Version, Error> {
    let text = std::str::from_utf8(output)
        .map_err(|_| Error::Version)?
        .trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut parts = text.split('.');
    let mut number = || {
        let digits = parts.next().ok_or(Error::Version)?;
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::Version);
        }
        digits.parse::<u64>().map_err(|_| Error::Version)
    };
    let version = Version(number()?, number()?, number()?);
    if parts.next().is_some() {
        return Err(Error::Version);
    }
    Ok(version)
}
