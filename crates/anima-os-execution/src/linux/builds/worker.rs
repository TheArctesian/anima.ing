//! Runs only inside the rootless build service's private filesystem and PID space.
use super::super::{Error, Result};
use super::output::{
    progress::{self, Stage},
    run as run_command,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    Dockerfile,
    Railpack,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: u32,
    pub strategy: Strategy,
    pub frontend: String,
}

pub async fn run() -> Result<()> {
    let mapping = fs::read_to_string("/proc/self/uid_map")?;
    let first: Vec<_> = mapping
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect();
    if !nix::unistd::Uid::effective().is_root()
        || first.len() != 3
        || first[0] != "0"
        || first[1] == "0"
        || first[2] != "1"
    {
        return Err(Error::Refused(
            "The build worker requires its isolated rootless namespace.",
        ));
    }
    let request: Request = read("/etc/anima/build-request.json", 16384)?;
    if request.schema != 1
        || !request
            .frontend
            .starts_with("ghcr.io/railwayapp/railpack-frontend@sha256:")
    {
        return Err(Error::Refused(
            "The pinned build toolchain request is invalid.",
        ));
    }
    for directory in ["/work/home", "/work/cache", "/work/plan", "/work/run"] {
        fs::create_dir_all(directory)?;
    }
    progress::write(Path::new("/work"), Stage::WaitingForBuilder, false)?;
    let mut daemon = Command::new("buildkitd")
        .args([
            "--root",
            "/work/buildkit",
            "--addr",
            "unix:///work/buildkit.sock",
            "--oci-worker-snapshotter",
            "native",
            "--containerd-worker=false",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let result = execute(&request).await;
    let _ = daemon.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(10), daemon.wait()).await;
    if result.is_err() {
        progress::failed(Path::new("/work"))?;
    }
    record_result(&result, request.strategy)?;
    result.map(drop)
}

fn record_result(result: &Result<Value>, strategy: Strategy) -> Result<()> {
    let value = match result {
        Ok(value) => value.clone(),
        Err(_) => json!({"schema":1,"phase":"failed","strategy":strategy}),
    };
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/work/result.pending")?;
    output.write_all(&serde_json::to_vec(&value)?)?;
    output.sync_all()?;
    fs::rename("/work/result.pending", "/work/result.json")?;
    fs::File::open("/work")?.sync_all()?;
    Ok(())
}

async fn execute(request: &Request) -> Result<Value> {
    wait_for_builder().await?;
    build(request).await?;
    progress::write(Path::new("/work"), Stage::CheckingExport, false)?;
    exported(request.strategy)
}

async fn wait_for_builder() -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if run_command(
            "buildctl",
            &["--addr", "unix:///work/buildkit.sock", "debug", "workers"],
            2,
            Stage::WaitingForBuilder,
        )
        .await
        .is_ok()
        {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Refused("The private builder did not become ready."));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(())
}

async fn build(request: &Request) -> Result<()> {
    let mut arguments = vec![
        "--addr",
        "unix:///work/buildkit.sock",
        "build",
        "--local",
        "context=/source",
        "--output",
        "type=oci,dest=/work/image.oci,name=localhost/anima/build:artifact,oci-artifact=false",
        "--metadata-file",
        "/work/metadata.json",
        "--opt",
        "attest:provenance=mode=min,version=v0.2",
    ];
    let frontend = format!("source={}", request.frontend);
    match request.strategy {
        Strategy::Dockerfile => arguments.extend([
            "--frontend",
            "dockerfile.v0",
            "--local",
            "dockerfile=/source",
        ]),
        Strategy::Railpack => {
            progress::write(Path::new("/work"), Stage::Detecting, false)?;
            run_command(
                "railpack",
                &[
                    "prepare",
                    "/source",
                    "--plan-out",
                    "/work/plan/railpack-plan.json",
                    "--info-out",
                    "/work/plan/railpack-info.json",
                ],
                300,
                Stage::Detecting,
            )
            .await?;
            arguments.extend([
                "--frontend",
                "gateway.v0",
                "--opt",
                &frontend,
                "--local",
                "dockerfile=/work/plan",
                "--opt",
                "filename=railpack-plan.json",
            ]);
        }
    }
    progress::write(Path::new("/work"), Stage::Building, false)?;
    run_command("buildctl", &arguments, 3600, Stage::Building).await
}

fn exported(strategy: Strategy) -> Result<Value> {
    let metadata: Value = read("/work/metadata.json", 1_048_576)?;
    let digest = metadata["containerimage.digest"]
        .as_str()
        .filter(|value| {
            value.len() == 71
                && value.starts_with("sha256:")
                && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or(Error::Refused(
            "The builder did not return an immutable image digest.",
        ))?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/work/image.oci")?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > 8 * 1024_u64.pow(3) {
            return Err(Error::Refused("The exported artifact exceeded its limit."));
        }
        hash.update(&buffer[..count]);
    }
    // The durable result must never precede the artifact bytes it names.
    file.sync_all()?;
    Ok(
        json!({"schema":1,"phase":"exported","strategy":strategy,"manifestDigest":digest,
        "archiveDigest":format!("sha256:{}", crate::linux::digests::hex(&hash.finalize())),"bytes":bytes,"deployed":false}),
    )
}

fn read<T: serde::de::DeserializeOwned>(path: &str, limit: u64) -> Result<T> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Refused("Build metadata exceeded its limit."));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
