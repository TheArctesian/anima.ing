//! The end of the holder: it ends each program it holds, and a new holder ends what a killed one
//! left (THR-049, H9).

use anima_daemon::threads::tree;
use anima_daemon::threads::wire::{Event, Outcome};

use super::super::*;

#[tokio::test]
async fn thr_049_the_end_of_the_holder_ends_its_programs_and_a_new_holder_ends_what_a_killed_one_left()
 {
    let mut holder = Holder::start(0);
    let mut link = holder.link().await;
    let id = link
        .send(start(
            "a",
            "/bin/sh",
            &[
                "-c",
                "perl -e 'setpgrp(0,0); sleep 300' & echo $!; sleep 300",
            ],
        ))
        .await;
    let Outcome::Started { pid } = link.answer(id).await else {
        panic!("not started")
    };
    let child: u32 = texts(
        &link
            .until(|event| matches!(event, Event::Lines { .. }))
            .await,
    )[0]
    .1
    .parse()
    .expect("a pid");
    holder.child.kill().expect("SIGKILL");
    holder.child.wait().expect("ended");
    assert!(tree::alive(pid), "a killed holder ends nothing by itself");
    holder.child = spawn(&holder.dir, 0);
    let _link = holder.link().await;
    assert!(
        gone(pid).await && gone(child).await,
        "the new holder ended what the old one left"
    );

    let mut link = holder.link().await;
    let id = link.send(start("b", "/bin/sh", &["-c", "sleep 300"])).await;
    let Outcome::Started { pid } = link.answer(id).await else {
        panic!("not started")
    };
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(i32::try_from(holder.child.id()).expect("pid"))
            .expect("pid"),
        rustix::process::Signal::TERM,
    )
    .expect("SIGTERM");
    holder.child.wait().expect("the holder ends");
    assert!(gone(pid).await, "SIGTERM to the holder ends its programs");
}

#[tokio::test]
async fn thr_049_the_end_of_the_holder_ends_its_threads_together_not_one_after_another() {
    let mut holder = Holder::start(0);
    let mut link = holder.link().await;
    let mut pids = Vec::new();
    for thread in ["one", "two", "three"] {
        // Each program waits out its whole time after `SIGTERM`, also when its child is gone.
        let script = "trap '' TERM; while :; do sleep 1; done";
        let id = link.send(start(thread, "/bin/sh", &["-c", script])).await;
        let Outcome::Started { pid } = link.answer(id).await else {
            panic!("not started")
        };
        pids.push(pid);
    }
    let begun = std::time::Instant::now();
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(i32::try_from(holder.child.id()).expect("pid"))
            .expect("pid"),
        rustix::process::Signal::TERM,
    )
    .expect("SIGTERM");
    holder.child.wait().expect("the holder ends");
    assert!(
        begun.elapsed() < std::time::Duration::from_secs(8),
        "three threads that wait out their time end together: {:?}",
        begun.elapsed()
    );
    for pid in pids {
        assert!(gone(pid).await, "a program is left");
    }
}
