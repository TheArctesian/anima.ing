use std::os::unix::process::CommandExt as _;
use std::time::Duration;

use rustix::process::{DumpableBehavior, set_dumpable_behavior};
use tokio::time::{Instant, sleep, timeout};

use super::super::*;
use super::{PendingChild, pending};
use crate::remote_code::{
    Store,
    launch::{Progress, pending_for_shutdown},
};
use crate::threads::{link::HolderLink, tree::identity::Process, wire::Action};

#[tokio::test]
async fn th32_holder_restart_recovers_cleanup_after_shutdown_cannot_inspect_a_descendant() {
    let program = std::env::current_exe().unwrap();
    let (fixture, home, request, child) = pending_for_shutdown(
        &program,
        &[
            "--ignored",
            "--exact",
            "threads::holder::remote_code::cleanup_tests::provider_fixture",
        ],
    );
    let dir = home.dir().join("threads");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(
        &dir,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
    )
    .unwrap();
    let shared = holder_state(&dir, 0, Vec::new());
    assert!(pending::reserve(&shared, request.runner));
    locked(&shared).remote_child = Some(PendingChild {
        request,
        child,
        since: Instant::now(),
        ending: false,
    });
    let listener = bind(&dir).unwrap();
    let first = tokio::spawn(serve_here(dir.clone(), listener, shared));
    let descendant = descendant(fixture.path()).await;
    assert!(
        std::fs::read(format!("/proc/{}/environ", descendant.0.identity().pid)).is_err(),
        "the fixture must exercise a real permission failure"
    );
    quit(&dir).await;
    timeout(Duration::from_secs(6), first)
        .await
        .unwrap()
        .unwrap();
    assert!(
        descendant.0.live(),
        "the first holder could not finish cleanup"
    );

    std::fs::write(fixture.path().join("readable"), []).unwrap();
    let mut second = tokio::process::Command::new(program)
        .args([
            "--ignored",
            "--exact",
            "threads::holder::remote_code::cleanup_tests::restart_fixture",
        ])
        .current_dir(&dir)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    timeout(Duration::from_secs(5), async {
        while descendant.0.live() {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the restarted holder should finish cleanup");
    quit(&dir).await;
    assert!(
        timeout(Duration::from_secs(6), second.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let saved = Store::at(&home).reservation(&request).unwrap();
    assert_eq!(saved.progress(), Progress::Reconciling);
    assert!(saved.evidence().is_none());
}

async fn serve_here(dir: PathBuf, listener: UnixListener, shared: Arc<Mutex<Shared>>) {
    serve_holder(&dir, listener, shared).await.unwrap();
}

#[tokio::test]
#[ignore = "fresh holder process for the shutdown regression"]
async fn restart_fixture() {
    let dir = std::env::current_dir().unwrap();
    hold(&dir, 0).await.unwrap();
}

async fn quit(dir: &Path) {
    let mut link = timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(link) = HolderLink::connect(dir).await {
                return link;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    link.send(Action::Quit).await.unwrap();
    timeout(Duration::from_secs(6), task)
        .await
        .unwrap()
        .unwrap();
}

struct Descendant(Process);
impl Drop for Descendant {
    fn drop(&mut self) {
        let _ = self.0.signal(rustix::process::Signal::KILL);
    }
}

async fn descendant(dir: &Path) -> Descendant {
    timeout(Duration::from_secs(5), async {
        loop {
            if let Some(process) = std::fs::read_to_string(dir.join("descendant"))
                .ok()
                .and_then(|pid| pid.parse().ok())
                .and_then(Process::open)
            {
                return Descendant(process);
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}

#[test]
#[ignore = "synthetic provider for the holder shutdown regression"]
fn provider_fixture() {
    let mut descendant = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "threads::holder::remote_code::cleanup_tests::descendant_fixture",
        ])
        .process_group(0)
        .spawn()
        .unwrap();
    let _ = descendant.wait();
}

#[test]
#[ignore = "synthetic escaped descendant for the holder shutdown regression"]
fn descendant_fixture() {
    set_dumpable_behavior(DumpableBehavior::NotDumpable).unwrap();
    std::fs::write("descendant", std::process::id().to_string()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !Path::new("readable").exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    set_dumpable_behavior(DumpableBehavior::Dumpable).unwrap();
    std::thread::sleep(Duration::from_secs(15));
}
