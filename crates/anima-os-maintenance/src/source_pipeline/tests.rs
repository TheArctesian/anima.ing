use super::*;
use crate::{
    Lease, Operation,
    capacity::Resources,
    drain, observation,
    reservations::{self, Reservation, RuntimeOwner},
};
use std::{
    io::Write as _,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    time::SystemTime,
};

#[tokio::test]
#[ignore = "requires an isolated root Linux container with empty /run/anima-os"]
async fn draining_holds_new_jobs_and_power_while_the_accepted_pipeline_changes_stage() {
    fs::DirBuilder::new().mode(0o700).create(ROOT).unwrap();
    fs::create_dir_all("/persist/anima-admission/reservations").unwrap();
    let admission = Admission::cleanup().unwrap();
    crate::recovery::complete(&admission).unwrap();
    record_pressure_observation(&admission);
    let execution = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let build = Uuid::now_v7();
    Pipeline::accept(&admission, (execution, workspace, build)).unwrap();
    drop(admission);
    check_activation_window(execution);
    drain::begin().unwrap();

    let impact = serde_json::to_value(observation::impact().await.unwrap()).unwrap();
    assert_eq!(impact["reservations"], 0);
    assert_eq!(impact["sourceInProgress"], true);
    assert_eq!(impact["ready"], false);
    assert!(matches!(Lease::prepare(Operation::Restart).await,
        Err(Error::Refused(message)) if message.contains("accepted source build")));

    let admission = Admission::acquire().unwrap();
    assert!(
        matches!(Pipeline::accept(&admission, (Uuid::now_v7(), workspace, build)),
        Err(Error::Refused(message)) if message.contains("draining work"))
    );
    drop(admission);

    let pipeline = Pipeline::resume(execution).unwrap();
    assert!(Pipeline::resume(Uuid::now_v7()).is_err());
    let admission = Admission::for_pipeline(Some(pipeline)).unwrap();
    assert!(admission.require_open_work().is_ok());
    let unrelated = Reservation {
        schema: 1,
        runtime: RuntimeOwner::SystemdBuild,
        run: Uuid::now_v7(),
        generation: 1,
        resources: Resources {
            memory_bytes: 512 << 20,
            cpu_millis: 500,
            tasks: 128,
        },
    };
    assert!(
        matches!(reservations::reserve(&admission, unrelated.clone()),
        Err(Error::Refused(message)) if message.contains("does not match source pipeline"))
    );
    check_held_dispatch(
        &admission,
        Reservation {
            run: build,
            ..unrelated
        },
    );

    retire(&admission, Uuid::now_v7()).unwrap();
    assert_eq!(Pipeline::resume(execution).unwrap(), pipeline);
    retire(&admission, execution).unwrap();
    drop(admission);
    assert!(Admission::for_pipeline(Some(pipeline)).is_err());
    assert!(active().unwrap().is_none());
}

fn check_activation_window(execution: Uuid) {
    let directory = "/persist/anima-updates";
    fs::DirBuilder::new().mode(0o700).create(directory).unwrap();
    let path = format!("{directory}/activation-window.json");
    let state = format!("{directory}/state.json");
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    let mut staged = serde_json::json!({
        "phase": "staged", "version": "20261126000000", "system": "/nix/store/candidate",
        "previous": "/nix/store/previous",
    });
    let mut window = serde_json::json!({
        "id": Uuid::now_v7(), "version": "20261126000000", "review": "a".repeat(64),
        "startsAt": now + 120, "endsAt": now + 360, "phase": "scheduled", "reason": null,
        "drain": {"boot": boot.trim(), "system": staged["system"], "previous": staged["previous"]},
    });
    let save = |window: &serde_json::Value| write_private(&path, window);
    write_private(&state, &staged);
    save(&window);
    assert!(Admission::acquire().unwrap().require_open_work().is_ok());
    window["startsAt"] = (now - 120).into();
    save(&window);
    assert!(
        matches!(Admission::acquire().unwrap().require_open_work(), Err(Error::Refused(message))
            if message.contains("activation window"))
    );
    assert_eq!(
        serde_json::to_value(crate::work_admission().unwrap()).unwrap(),
        "update-window"
    );
    let pipeline = Pipeline::resume(execution).unwrap();
    assert!(
        Admission::for_pipeline(Some(pipeline))
            .unwrap()
            .require_open_work()
            .is_ok()
    );
    assert!(crate::hosting::available().is_ok());
    let earlier = Admission {
        opened_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(now - 121),
        ..Admission::acquire().unwrap()
    };
    assert!(earlier.require_open_work().is_ok());
    drop(earlier);
    staged["system"] = "/nix/store/other".into();
    write_private(&state, &staged);
    assert!(Admission::acquire().unwrap().require_open_work().is_ok());
    staged["system"] = window["drain"]["system"].clone();
    write_private(&state, &staged);

    check_window_exit(execution, window, save, now);
    fs::remove_file(path).unwrap();
    fs::remove_file(state).unwrap();
    fs::remove_dir(directory).unwrap();
}

fn check_window_exit(
    execution: Uuid,
    mut window: serde_json::Value,
    save: impl Fn(&serde_json::Value),
    now: u64,
) {
    window["endsAt"] = (now - 1).into();
    save(&window);
    assert!(Admission::acquire().unwrap().require_open_work().is_ok());
    window["endsAt"] = (now + 360).into();
    let original = window["drain"].clone();
    window["drain"]["boot"] = execution.to_string().into();
    save(&window);
    assert!(Admission::acquire().unwrap().require_open_work().is_ok());
    window.as_object_mut().unwrap().remove("drain");
    save(&window);
    assert!(Admission::acquire().unwrap().require_open_work().is_ok());
    window["drain"] = original;
    save(&window);
    let manual = serde_json::to_value(drain::begin().unwrap()).unwrap();
    window["phase"] = "cancelled".into();
    save(&window);
    assert!(
        matches!(Admission::acquire().unwrap().require_open_work(), Err(Error::Refused(message))
        if message.contains("draining work"))
    );
    drain::cancel(manual["id"].as_str().unwrap().parse().unwrap()).unwrap();
    assert!(Admission::acquire().unwrap().require_open_work().is_ok());
}

fn write_private(path: &str, value: &serde_json::Value) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
}

fn check_held_dispatch(admission: &Admission, reserved: Reservation) {
    let pressure = "/run/anima-hosting-pressure";
    fs::write(format!("{pressure}/active"), "synthetic hold").unwrap();

    // The launch recheck sees this exact reservation after staging the worker.
    let path = format!(
        "/persist/anima-admission/reservations/{}.json",
        reserved.run
    );
    let receipt = serde_json::to_vec(&reserved).unwrap();
    fs::write(&path, &receipt).unwrap();
    let dispatch = reservations::reserve(admission, reserved);
    assert!(
        matches!(&dispatch, Err(Error::HostingPressure)),
        "{dispatch:?}"
    );
    assert_eq!(fs::read(&path).unwrap(), receipt);
    fs::remove_file(path).unwrap();
    fs::remove_dir_all(pressure).unwrap();
}

fn record_pressure_observation(admission: &Admission) {
    let availability = admission.require_open_work();
    assert!(
        matches!(&availability, Err(Error::PressureUnavailable)),
        "{availability:?}"
    );
    let pressure = "/run/anima-hosting-pressure";
    fs::DirBuilder::new().mode(0o700).create(pressure).unwrap();
    let sampled_at = nix::time::clock_gettime(nix::time::ClockId::CLOCK_BOOTTIME)
        .unwrap()
        .tv_sec();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(format!("{pressure}/clear"))
        .unwrap();
    write!(file, "{sampled_at}").unwrap();
}
