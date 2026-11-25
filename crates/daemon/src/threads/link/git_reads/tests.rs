use super::*;

fn hello(roots: Option<Vec<GitReadRoot>>) -> Event {
    Event::Hello {
        version: crate::threads::wire::VERSION,
        threads: Vec::new(),
        build: None,
        git_read_roots: roots,
    }
}

fn root(generation: u64) -> GitReadRoot {
    GitReadRoot {
        thread: uuid::Uuid::from_u128(1).to_string(),
        process: crate::threads::tree::identity::Identity {
            pid: 41,
            device: 1,
            inode: generation,
        },
        generation,
    }
}

#[test]
fn tc7_w3_older_holder_never_enrolls_a_root() {
    let holder = GitReads::new(Process::open(std::process::id()));
    holder.observe(&hello(None));
    holder.observe(&Event::GitReadRoot {
        root: root(1),
        enrolled: true,
    });
    assert!(LiveRoots::of(&holder).is_none());
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
#[test]
fn tc7_w3_suppressed_reaped_or_disconnected_launch_cannot_deliver_a_pending_result() {
    let Some(process) = Process::open(std::process::id()) else {
        eprintln!("Kernel has no pidfs support; live grants are unavailable.");
        return;
    };
    let first = root(1);
    let holder = GitReads::new(Some(process.clone()));
    holder.observe(&hello(Some(vec![first.clone()])));
    let pending = LiveRoots::of(&holder).expect("the negotiated holder");
    assert_eq!(
        pending.with_current(&holder, &first, || "sent"),
        Some("sent")
    );
    holder.suppress(&first.thread);
    assert!(
        pending
            .with_current(&holder, &first, || panic!("stopped reply"))
            .is_none()
    );
    holder.observe(&Event::GitReadRoot {
        root: first.clone(),
        enrolled: false,
    });
    assert!(
        pending
            .with_current(&holder, &first, || panic!("reaped reply"))
            .is_none()
    );

    let second = root(2);
    holder.observe(&Event::GitReadRoot {
        root: second.clone(),
        enrolled: true,
    });
    let running = LiveRoots::of(&holder).expect("the new launch");
    assert!(
        pending
            .with_current(&holder, &first, || panic!("stale same-thread reply"))
            .is_none()
    );
    assert_eq!(
        running.with_current(&holder, &second, || "sent"),
        Some("sent")
    );
    holder.disconnect();
    assert!(
        running
            .with_current(&holder, &second, || panic!("disconnected reply"))
            .is_none()
    );

    let reconnected = GitReads::new(Some(process));
    reconnected.observe(&hello(Some(vec![second.clone()])));
    assert!(
        running
            .with_current(&reconnected, &second, || panic!("old connection reply"))
            .is_none()
    );
    let fresh = LiveRoots::of(&reconnected).expect("fresh Hello re-enrolls survivor");
    assert_eq!(
        fresh.with_current(&reconnected, &second, || "sent"),
        Some("sent")
    );
}
