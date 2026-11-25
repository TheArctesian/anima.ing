use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal},
};

use super::*;

#[tokio::test]
async fn th32_pending_creation_failure_ends_the_forked_group_before_reaping() {
    let (_dir, home, claim, device) = launch();
    let store = Store::at(&home);
    let saved = store
        .reserve((device, &Origin::default_hub()), &claim)
        .unwrap();
    let lease = PreparingReport(ticks(80_000)).accepted(*saved.request());

    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "/bin/sleep 30 & printf '%s\\n' \"$!\" > descendant-pid",
        ])
        .current_dir(&saved.parameters().folder)
        .process_group(0);
    let Dispatch::Spawned(mut child) = store
        .spawn_with_clock(
            (grant(&saved), &lease),
            (|_| Ok(Mode::Ask), || Some(ticks(201_000))),
            (Mode::Ask, command),
        )
        .unwrap()
    else {
        panic!("the synthetic fork did not start");
    };

    let unavailable = store.lock().unwrap();
    let descendant = Descendant::of(&saved.parameters().folder).await;
    assert!(matches!(child.record(&store), Err(StoreError::Busy)));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !child.exited().unwrap() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the group leader should have exited");
    child.end().await.unwrap();
    assert!(
        descendant.ended().await,
        "a forked process survived the pending launch cleanup"
    );

    drop(unavailable);
    let reopened = Store::at(&home).reservation(saved.request()).unwrap();
    assert_eq!(reopened.progress(), Progress::Reconciling);
    assert!(reopened.evidence().is_none());
}

struct Descendant(OwnedFd);

impl Descendant {
    async fn of(folder: &Path) -> Self {
        let pid_file = folder.join("descendant-pid");
        tokio::time::timeout(Duration::from_secs(2), async move {
            loop {
                if let Some(pid) = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|text| text.trim().parse().ok())
                    .and_then(Pid::from_raw)
                {
                    return Self(pidfd_open(pid, PidfdFlags::empty()).unwrap());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the group leader should publish its synthetic child's PID")
    }

    async fn ended(&self) -> bool {
        tokio::time::timeout(Duration::from_secs(2), async {
            while matches!(
                poll(
                    &mut [PollFd::new(&self.0, PollFlags::IN)],
                    Some(&Timespec::default())
                ),
                Ok(0)
            ) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .is_ok()
    }
}

impl Drop for Descendant {
    fn drop(&mut self) {
        let _ = pidfd_send_signal(&self.0, Signal::KILL);
    }
}
