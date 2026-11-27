use super::{Admission, DIRECTORY, Error, Reservation, RuntimeOwner, path, read};
use std::{fs, path::PathBuf, process::Stdio, time::Duration};
use uuid::Uuid;

const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Release a build reservation only after systemd and its cgroup prove it ended.
/// # Errors
/// A conflicting generation, running unit, populated cgroup or unknown state.
pub async fn release_build(_guard: &Admission, run: Uuid, generation: u64) -> Result<(), Error> {
    let present = path(run).try_exists()?;
    if present {
        let reservation: Reservation = read(&path(run))?;
        if reservation.generation != generation || reservation.runtime != RuntimeOwner::SystemdBuild
        {
            return Err(Error::Refused("The build reservation identity changed."));
        }
    }
    let unit = format!("anima-build-{run}.service");
    terminated(&unit).await?;
    if present {
        fs::remove_file(path(run))?;
    }
    fs::File::open(DIRECTORY)?.sync_all()?;
    Ok(())
}

async fn terminated(unit: &str) -> Result<(), Error> {
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new("/run/current-system/sw/bin/systemctl")
            .args([
                "show",
                unit,
                "--property=LoadState,ActiveState,MainPID,ControlPID,ControlGroup",
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| Error::Refused("Build termination observation timed out."))??;
    if result.stdout.len() > 16384 {
        return Err(Error::Refused("Build termination could not be observed."));
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|_| Error::Refused("Invalid build unit state."))?;
    let events = ended_cgroup(unit, text, result.status.success())?;
    match fs::read_to_string(events) {
        Ok(events) if events.lines().any(|line| line == "populated 0") => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::metadata(format!("{CGROUP_ROOT}/cgroup.controllers"))?;
        }
        _ => {
            return Err(Error::Refused(
                "Build processes have not left their cgroup.",
            ));
        }
    }
    Ok(())
}

fn ended_cgroup(unit: &str, text: &str, observed: bool) -> Result<PathBuf, Error> {
    let value = |key: &str| text.lines().find_map(|line| line.strip_prefix(key));
    if !observed && value("LoadState=") != Some("not-found") {
        return Err(Error::Refused("The build unit could not be observed."));
    }
    if !matches!(value("ActiveState="), Some("inactive" | "failed"))
        || value("MainPID=") != Some("0")
        || value("ControlPID=") != Some("0")
    {
        return Err(Error::Refused("The build still owns running processes."));
    }
    let group =
        value("ControlGroup=").ok_or(Error::Refused("Build cgroup state is unavailable."))?;
    // Systemd places a dash-named slice under the slice named by its prefix.
    let expected = format!("/anima.slice/anima-agents.slice/{unit}");
    if !group.is_empty() && group != expected {
        return Err(Error::Refused("The build cgroup identity changed."));
    }
    Ok(PathBuf::from(format!(
        "{CGROUP_ROOT}{expected}/cgroup.events"
    )))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::ended_cgroup;

    #[test]
    fn exited_builds_and_unloaded_units_both_check_the_packaged_agent_slice() {
        let unit = "anima-build-00000000-0000-7000-8000-000000000001.service";
        let group = format!("/anima.slice/anima-agents.slice/{unit}");
        let events = format!("/sys/fs/cgroup{group}/cgroup.events");
        for (state, reported, observed) in
            [("loaded", group.as_str(), true), ("not-found", "", false)]
        {
            let properties = format!(
                "LoadState={state}\nActiveState=inactive\nMainPID=0\nControlPID=0\nControlGroup={reported}\n"
            );
            assert_eq!(
                ended_cgroup(unit, &properties, observed).unwrap(),
                Path::new(&events)
            );
        }
        let other = format!(
            "LoadState=loaded\nActiveState=inactive\nMainPID=0\nControlPID=0\nControlGroup=/system.slice/{unit}\n"
        );
        assert!(ended_cgroup(unit, &other, true).is_err());
    }
}
