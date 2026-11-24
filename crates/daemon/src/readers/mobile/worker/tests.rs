use std::path::Path;
use std::time::{Duration, Instant};

use daemon_core::inventory::mobile::{Probe, State, Version};
use rustix::process::Pid;
use tokio::process::Command;

// Fixtures self-terminate after 30 seconds even if an assertion fails. Never
// signal a PID read from a file: it may have exited and been reused.
struct Worker {
    root: tempfile::TempDir,
}

impl Worker {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn command(&self, ending: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                &format!("echo $$ > worker; /bin/sleep 30 & echo $! > descendant; {ending}"),
            ])
            .env_clear()
            .current_dir(self.root.path());
        command
    }

    async fn assert_stopped(&self) {
        // Check both the worker and its stdout-holding child. A reparented zombie
        // has already closed its descriptors; its reaping belongs to the OS.
        for name in ["worker", "descendant"] {
            let pid = pid(self.root.path(), name);
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                let output = Command::new("/bin/ps")
                    .args(["-o", "stat=", "-p", &pid.as_raw_nonzero().to_string()])
                    .env_clear()
                    .output()
                    .await
                    .unwrap();
                let status = String::from_utf8(output.stdout).unwrap();
                if status.trim().is_empty() || status.trim().starts_with('Z') {
                    std::fs::remove_file(self.root.path().join(name)).unwrap();
                    break;
                }
                assert!(Instant::now() < deadline, "{name} still running: {status}");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

fn pid(root: &Path, name: &str) -> Pid {
    let text = std::fs::read_to_string(root.join(name)).unwrap();
    Pid::from_raw(text.trim().parse().unwrap()).unwrap()
}

#[tokio::test]
async fn blocked_worker_times_out_at_five_seconds_and_ends_its_descendants() {
    let worker = Worker::new();
    let mut command = worker.command("wait");
    let started = Instant::now();
    let result: Probe<Version> =
        tokio::time::timeout(Duration::from_secs(8), super::collect_probe(&mut command))
            .await
            .expect("the parent must return within its deadline and scheduling allowance");
    let elapsed = started.elapsed();
    assert_eq!(result, Probe::Unavailable(State::TimedOut));
    assert!(elapsed >= Duration::from_millis(4500), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(8), "{elapsed:?}");
    worker.assert_stopped().await;
}

#[tokio::test]
async fn exited_worker_with_descendant_holding_stdout_returns_and_cleans_up() {
    let worker = Worker::new();
    let mut command =
        worker.command("printf '%s' '{\"state\":\"read\",\"version\":\"35.2\"}'; exit 0");
    let result: Probe<Version> =
        tokio::time::timeout(Duration::from_secs(3), super::collect_probe(&mut command))
            .await
            .expect("a descendant must not hold the completed worker's result open");
    assert_eq!(
        result,
        Probe::Read(Version {
            version: "35.2".into()
        })
    );
    worker.assert_stopped().await;
}

#[tokio::test(start_paused = true)]
async fn parent_collection_times_out_at_exactly_five_seconds() {
    use std::future::Future as _;
    use std::task::{Context, Poll, Waker};

    let worker = Worker::new();
    let mut command = worker.command("wait");
    let mut collection = std::pin::pin!(super::collect_probe::<Version>(&mut command));
    let mut context = Context::from_waker(Waker::noop());
    assert!(collection.as_mut().poll(&mut context).is_pending());
    tokio::time::advance(Duration::from_millis(4999)).await;
    assert!(collection.as_mut().poll(&mut context).is_pending());
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(
        collection.as_mut().poll(&mut context),
        Poll::Ready(Probe::Unavailable(State::TimedOut))
    );
    // This checks the collection deadline, not deterministic isolation of the
    // outer guard from the inner timeout and OS reaper scheduling.
    tokio::time::resume();
}
