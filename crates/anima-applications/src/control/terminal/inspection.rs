use std::{io, path::Path, time::Instant};

use serde::Deserialize;

use crate::{
    Paths,
    execution::{Fault, check_socket, files::Store, process},
    forms::is_engine_name,
};

use super::Error;

const FORMAT: &str = concat!(
    r#"{{$workspace := false}}{{range $key, $_ := .Config.Labels}}{{if eq $key "ing.anima.workspace"}}{{$workspace = true}}{{end}}{{end}}"#,
    r#"{"id":{{json .Id}},"name":{{json .Name}},"running":{{and .State.Running (not .State.Paused) (not .State.Restarting)}},"#,
    r#""workspace":{{$workspace}},"project":{{json (index .Config.Labels "com.docker.compose.project")}},"#,
    r#""service":{{json (index .Config.Labels "com.docker.compose.service")}},"image":{{json .Config.Image}},"user":{{json .Config.User}},"#,
    r#""folder":{{json (index .Config.Labels "com.docker.compose.project.working_dir")}},"#,
    r#""config_hash":{{json (index .Config.Labels "com.docker.compose.config-hash")}},"#,
    r#""final_form":{{json (index .Config.Labels "com.docker.compose.project.config_files")}},"#,
    r#""isolated":{{and (not .HostConfig.Privileged) (not .HostConfig.CapAdd) (not .HostConfig.Devices) (not .HostConfig.DeviceRequests)"#,
    r#" (not .HostConfig.DeviceCgroupRules) (not .HostConfig.VolumesFrom)"#,
    r#" (eq .HostConfig.PidMode "") (eq .HostConfig.UTSMode "") (eq .HostConfig.UsernsMode "")}},"#,
    r#""network":{{json .HostConfig.NetworkMode}},"ipc":{{json .HostConfig.IpcMode}},"#,
    r#""security":{{json .HostConfig.SecurityOpt}},"mounts":{{if .Mounts}}{{json .Mounts}}{{else}}[]{{end}}}"#,
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inspection {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub workspace: bool,
    pub project: Option<String>,
    pub service: Option<String>,
    pub image: String,
    pub user: String,
    pub folder: Option<String>,
    pub final_form: Option<String>,
    pub config_hash: Option<String>,
    isolated: bool,
    network: String,
    ipc: String,
    security: Option<Vec<String>>,
    mounts: Vec<Mount>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Mount {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    driver: String,
}

pub(super) fn read(paths: &Paths, id: &str, deadline: Instant) -> Result<Inspection, Fault> {
    check_socket(paths)?;
    let command = paths.docker(&["container", "inspect", "--format", FORMAT], &[id]);
    let bytes = process::capture_bounded(paths, &command, deadline, 32 * 1024)?;
    serde_json::from_slice(&bytes).map_err(|_| Fault::Process(io::ErrorKind::InvalidData))
}

impl Inspection {
    pub(super) fn isolated(&self) -> bool {
        self.isolated
            && self.network != "host"
            && !self.network.starts_with("container:")
            && matches!(self.ipc.as_str(), "" | "private" | "shareable" | "none")
            && self.security.as_ref().is_none_or(|options| {
                options.iter().all(|option| {
                    matches!(
                        option.as_str(),
                        "no-new-privileges" | "no-new-privileges:true" | "no-new-privileges=true"
                    )
                })
            })
    }

    pub(super) fn check_mounts(
        &self,
        store: &Store<'_>,
        paths: &Paths,
        (root, deadline): (&str, Instant),
    ) -> Result<(), Error> {
        let project_prefix = format!("{}_", self.project.as_deref().ok_or(Error::Target)?);
        for mount in &self.mounts {
            match mount.kind.as_str() {
                "bind" if Path::new(&mount.source).parent() == Some(Path::new(root)) => {
                    store.existing_bind(&mount.source)?;
                }
                "volume"
                    if mount.driver == "local"
                        && is_engine_name(&mount.name)
                        && mount.name.starts_with(&project_prefix) =>
                {
                    let options = paths.docker(
                        &["volume", "inspect", "--format", "{{json .Options}}"],
                        &[&mount.name],
                    );
                    check_socket(paths)?;
                    let bytes = process::capture_bounded(paths, &options, deadline, 2048)?;
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(|_| Error::Isolation)?;
                    if !value.is_null() && !value.as_object().is_some_and(serde_json::Map::is_empty)
                    {
                        return Err(Error::Isolation);
                    }
                }
                "tmpfs" => {}
                _ => return Err(Error::Isolation),
            }
        }
        Ok(())
    }
}
