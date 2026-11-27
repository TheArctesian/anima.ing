//! Packaged sandbox policy and the transient unit that enforces it.
use super::{
    super::{Error, Result},
    store,
};
use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Sandbox {
    schema: u32,
    root_directory: String,
    path: String,
    rootlesskit: String,
    certificates: String,
    worker_user: String,
}

impl Sandbox {
    pub(super) fn load() -> Result<(Self, Value)> {
        let sandbox: Sandbox = packaged("/etc/anima/build-sandbox.json")?;
        let toolchain: Value = packaged("/etc/anima/build-toolchain.json")?;
        if sandbox.schema != 1 || sandbox.worker_user != "anima-builder" || toolchain["schema"] != 1
        {
            return Err(Error::Refused(
                "The packaged builder configuration is incompatible.",
            ));
        }
        Ok((sandbox, toolchain))
    }
}
fn packaged<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    // NixOS publishes /etc configuration as symlinks into its immutable store.
    // Mutable attempt receipts still use the strict no-symlink reader directly.
    let target = fs::canonicalize(path)?;
    if !target.starts_with("/nix/store") {
        return Err(Error::Refused(
            "The build configuration is not from the packaged system.",
        ));
    }
    store::read(&target)
}

impl Sandbox {
    pub(super) fn arguments(
        &self,
        attempt: &store::Attempt,
        context: &Path,
    ) -> Result<Vec<String>> {
        let id = attempt.id;
        let resources = attempt.resources;
        let base = store::base(id);
        let work = base.join("work");
        let request = base.join("request.json");
        let binary = std::env::current_exe()?;
        let mut arguments = vec![
            format!("--unit={}", store::unit(id)),
            "--service-type=exec".into(),
            "--quiet".into(),
        ];
        // Per-file /proc masks prevent RootlessKit from mounting procfs for its
        // PID namespace. Host privilege stays with the unprivileged builder UID;
        // runc applies kernel-control masks inside each build container.
        let properties = vec![
        "User=anima-builder".into(), "Group=anima-builder".into(), "Slice=anima-agents.slice".into(),
        format!("MemoryMax={}",resources.memory_bytes), "MemorySwapMax=0".into(),
        format!("CPUQuota={}.{}%",resources.cpu_millis/10,resources.cpu_millis%10), format!("TasksMax={}",resources.tasks),
        "CPUWeight=10".into(), "IOWeight=10".into(), "Nice=15".into(), "IOSchedulingClass=idle".into(), "OOMScoreAdjust=500".into(),
        format!("RuntimeMaxSec={}", attempt.seconds.get().min(store::MAX_SECONDS)), "TimeoutStopSec=20".into(), "KillMode=control-group".into(),
        "StandardOutput=null".into(), "StandardError=null".into(), "UMask=0077".into(), "WorkingDirectory=/work".into(),
        format!("RootDirectory={}",self.root_directory), "MountAPIVFS=true".into(), "ProtectSystem=strict".into(),
        "ProtectHome=true".into(), "ProtectControlGroups=true".into(), "ProtectKernelTunables=false".into(), "ProtectKernelModules=true".into(),
        "PrivateDevices=true".into(), "PrivateTmp=true".into(), "ReadWritePaths=/work".into(),
        "DeviceAllow=/dev/net/tun rw".into(),
        format!("BindPaths={}:/work /dev/net/tun",work.display()),
        format!("BindReadOnlyPaths=/nix/store /run/wrappers /etc/passwd /etc/group /etc/subuid /etc/subgid {}:/source {}:/etc/anima/build-request.json",context.display(),request.display()),
        "IPAddressDeny=10.0.0.0/8 100.64.0.0/10 169.254.0.0/16 172.16.0.0/12 192.168.0.0/16 fc00::/7 fe80::/10 224.0.0.0/4 ff00::/8".into(),
    ];
        for property in properties {
            arguments.push(format!("--property={property}"));
        }
        for (key, value) in [
            ("PATH", self.path.as_str()),
            ("HOME", "/work/home"),
            ("XDG_CACHE_HOME", "/work/cache"),
            ("XDG_RUNTIME_DIR", "/work/run"),
            ("SSL_CERT_FILE", self.certificates.as_str()),
        ] {
            arguments.push(format!("--setenv={key}={value}"));
        }
        arguments.extend([
            "--".into(),
            self.rootlesskit.clone(),
            "--state-dir=/work/rootlesskit".into(),
            "--net=slirp4netns".into(),
            // Keep virtual DNS outside the private-destination deny ranges.
            "--cidr=192.0.2.0/24".into(),
            "--disable-host-loopback".into(),
            "--copy-up=/etc".into(),
            "--copy-up=/run".into(),
            "--pidns".into(),
            text(&binary)?.into(),
            "build-worker".into(),
        ]);
        Ok(arguments)
    }
}

fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or(Error::Refused("The build path is invalid."))
}

#[cfg(test)]
mod tests;
