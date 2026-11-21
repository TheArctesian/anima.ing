use std::time::Duration;

use crate::model_work::cause::Cause;
use crate::model_work::names::Runtime;
use crate::model_work::releases::ArchiveFormat;
use crate::model_work::releases::fetch::{
    DONE, Ending, FetchOrder, SYSTEM_VARIABLE, child_deadline, download_deadline,
    environment_empty, redirect_permitted, release_hosts, reply, verdict,
};

fn order() -> FetchOrder {
    FetchOrder {
        runtime: "llama_server".to_owned(),
        address: "https://github.com/o/r/releases/download/b6000/llama.tar.gz".to_owned(),
        sha256: "a".repeat(64),
        size_bytes: 1 << 20,
        archive_folder: "/state".to_owned(),
        unpack_folder: "/runtimes/llama_server/.unpack-1".to_owned(),
        reserve: 0,
    }
}

#[test]
fn a_whole_order_gives_the_form_and_the_fixed_hosts_of_its_runtime() {
    let checked = order().checked().expect("holds");
    assert_eq!(checked.runtime, Runtime::LlamaServer);
    assert_eq!(checked.format, ArchiveFormat::TarGz);
    assert_eq!(checked.hosts, release_hosts(Runtime::LlamaServer));
}

#[test]
fn an_order_that_breaks_a_rule_is_refused_before_a_connection() {
    let broken: [fn(&mut FetchOrder); 7] = [
        |o| o.runtime = "curl".to_owned(),
        |o| o.address = o.address.replacen("https", "http", 1),
        |o| o.address = "https://github.com/a.exe".to_owned(),
        |o| o.sha256 = "A".repeat(64),
        |o| o.size_bytes = 0,
        |o| o.size_bytes = (8 << 30) + 1,
        |o| o.unpack_folder = "relative".to_owned(),
    ];
    for change in broken {
        let mut order = order();
        change(&mut order);
        assert_eq!(
            order.checked(),
            Err(Cause::RuntimeNotConfigured),
            "{order:?}"
        );
    }
}

#[test]
fn an_order_with_an_unknown_member_does_not_read() {
    let text = serde_json::to_value(order()).expect("writes");
    let mut text = text.as_object().expect("an object").clone();
    text.insert("token".to_owned(), serde_json::json!("x"));
    assert!(serde_json::from_value::<FetchOrder>(text.into()).is_err());
}

#[test]
fn a_redirect_reaches_only_a_named_host_over_https() {
    let hosts = release_hosts(Runtime::LlamaServer);
    assert!(redirect_permitted(
        "https",
        Some("objects.githubusercontent.com"),
        hosts
    ));
    assert!(!redirect_permitted("http", Some("github.com"), hosts));
    assert!(!redirect_permitted("https", Some("evil.example"), hosts));
    assert!(!redirect_permitted(
        "https",
        Some("140.82.112.3"),
        &["140.82.112.3"]
    ));
    assert!(!redirect_permitted("https", Some("[::1]"), &["[::1]"]));
    assert!(!redirect_permitted("https", None, hosts));
    assert_eq!(release_hosts(Runtime::Vllm), &[] as &[&str]);
}

#[test]
fn the_deadlines_grow_with_the_size_and_stop_at_a_limit() {
    assert_eq!(download_deadline(0), Duration::from_secs(300));
    assert_eq!(download_deadline(100 << 20), Duration::from_secs(400));
    assert_eq!(download_deadline(u64::MAX), Duration::from_secs(3_600));
    assert_eq!(child_deadline(0), Duration::from_mins(35));
}

#[test]
fn the_reply_of_the_child_reads_back_as_its_cause() {
    assert_eq!(
        verdict(Ending::Exited(&format!("{}\n", reply(Ok(()))))),
        Ok(())
    );
    for cause in [
        Cause::Checksum,
        Cause::Size,
        Cause::ArchiveRejected,
        Cause::LimitDisk,
    ] {
        assert_eq!(verdict(Ending::Exited(reply(Err(cause)))), Err(cause));
    }
    // A cause that the child may not give reads as `source_failed`.
    assert_eq!(reply(Err(Cause::Listener)), "source_failed");
    assert_eq!(
        verdict(Ending::Exited("listener")),
        Err(Cause::SourceFailed)
    );
    assert_eq!(
        verdict(Ending::Exited(&format!("{DONE} more"))),
        Err(Cause::SourceFailed)
    );
}

#[test]
fn a_fault_or_a_stall_of_the_child_is_source_failed() {
    assert_eq!(verdict(Ending::Fault), Err(Cause::SourceFailed));
    assert_eq!(verdict(Ending::Stalled), Err(Cause::SourceFailed));
}

#[test]
fn the_environment_of_the_child_is_empty_but_for_the_system_variable() {
    assert!(environment_empty(std::iter::empty::<&str>()));
    assert!(environment_empty([SYSTEM_VARIABLE].into_iter()));
    assert!(!environment_empty(
        [SYSTEM_VARIABLE, "HTTPS_PROXY"].into_iter()
    ));
    assert!(!environment_empty(["ANIMA_DAEMON_TOKEN"].into_iter()));
}
