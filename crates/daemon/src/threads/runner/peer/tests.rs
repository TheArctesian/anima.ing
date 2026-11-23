use super::*;

#[test]
fn thr_049_the_coalition_is_read_from_launchd() {
    let text = "pid/7 = {\n\ttype = pid\n\tresource coalition = {\n\t\tID = 87540\n\t\ttype = resource\n\t}\n\tjetsam coalition = {\n\t\tID = 87541\n\t}\n}\n";
    assert_eq!(coalition(text).as_deref(), Some("87540"));
    assert_eq!(coalition("pid/7 = {\n}\n"), None);
}

#[test]
fn thr_049_a_client_comes_from_the_runner_only_through_its_parents() {
    let rows = table(
        " 9 1 test\n 10 9 runner\n 11 9 anima agents approve\n 12 1 detached\n 13 12 child\n \
         14 10 claude --version\n 15 14 anima agents approve\n",
    );
    let line = BTreeSet::from([9, 10]);
    assert!(comes_from(&rows, 11, 10, &line), "started by the test");
    assert!(comes_from(&rows, 10, 10, &line), "the runner itself");
    assert!(
        !comes_from(&rows, 13, 10, &line),
        "the first process of the system ends the walk"
    );
    assert!(
        !comes_from(&rows, 15, 10, &line),
        "a program that the runner started is not the person"
    );
}

#[test]
fn thr_049_a_process_below_the_holder_is_of_an_agent_and_a_terminal_is_not() {
    let rows = table(
        "  1 0 /sbin/launchd\n 50 1 /Applications/Terminal\n 51 50 -zsh\n 52 51 anima agents approve x y\n \
         60 1 /u/anima-daemon run\n 61 60 /u/anima-daemon holder /s/threads\n 62 61 claude -p\n \
         63 62 /bin/bash -c anima agents approve a b\n 64 63 anima agents approve a b\n",
    );
    let mark = holder_mark(Path::new("/s/threads"));
    assert!(below_the_holder(&rows, 64, &mark));
    assert!(below_the_holder(&rows, 62, &mark));
    assert!(!below_the_holder(&rows, 52, &mark));
    assert!(
        !below_the_holder(&rows, 61, &mark),
        "the holder is not below itself"
    );
    assert!(!below_the_holder(&rows, 99, &mark), "an unknown process");
}

#[tokio::test]
async fn thr_049_a_client_that_cannot_be_read_is_taken_for_an_agent() {
    let dir = Path::new("/no/such/threads");
    assert!(of_an_agent(None, dir).await, "no process ID");
    assert!(of_an_agent(Some(0), dir).await, "process ID 0");
    let mut child = std::process::Command::new("/usr/bin/true")
        .spawn()
        .expect("a child");
    let gone = child.id();
    child.wait().expect("it ended");
    assert!(
        of_an_agent(i32::try_from(gone).ok(), dir).await,
        "a process that ended before it was read"
    );
    if std::env::var_os(crate::threads::marker::MARK).is_none() {
        let pid = std::process::id();
        // A system that keeps a read of this process from the runner (the Mac runner of
        // GitHub refused one on 2026-11-23) takes the test for an agent: the rule fails
        // closed. Where each read works, the test is the person.
        let marked = crate::threads::marker::mark_of(pid).await;
        let listed = commands().await.is_some_and(|rows| rows.contains_key(&pid));
        let grouped = !GROUPS_KNOWN || origin(pid).await.is_some();
        let readable = marked == Some(false) && listed && grouped;
        let me = i32::try_from(pid).ok();
        assert_eq!(
            of_an_agent(me, dir).await,
            !readable,
            "mark {marked:?}, in the table {listed}, group read {grouped}"
        );
    }
}

/// A process of an agent that left the tree of the holder (a new session, then a second
/// start, so that its parent is the first process of the system) and its mark (a new
/// environment): its parent and its environment no longer show what it is.
#[tokio::test]
async fn thr_049_a_process_that_left_the_holder_and_its_mark_is_still_of_an_agent() {
    let dir = tempfile::tempdir().expect("a folder");
    let pid_file = dir.path().join("pid");
    let holder_dir = dir.path().join("threads");
    let left = "use POSIX; my ($exe, $file) = @ARGV; \
                if (fork() == 0) { POSIX::setsid(); if (fork() == 0) { \
                open(my $h, '>', $file) or die; print $h $$; close $h; %ENV = (HOME => '/'); \
                exec {$exe} $exe, '--ignored', '--exact', \
                'threads::runner::peer::tests::thr_049_a_process_that_stays'; } exit 0; } \
                sleep 60;";
    let exe = std::env::current_exe().expect("the test program");
    let mut holder = std::process::Command::new("perl")
        .arg("-e")
        .arg(left)
        .arg(&exe)
        .arg(&pid_file)
        .arg("holder")
        .arg(&holder_dir)
        .spawn()
        .expect("a made holder");
    let mut left_pid = None;
    for _ in 0..100 {
        if let Some(pid) = std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|text| text.trim().parse::<i32>().ok())
        {
            left_pid = Some(pid);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let left_pid = left_pid.expect("the process that left");
    // Its parent is now the first process of the system (or the nearest that adopts).
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let seen = u32::try_from(left_pid).expect("a process ID");
    let table = commands().await.expect("the table");
    let group = origin(seen).await;
    let agent = of_an_agent(Some(left_pid), &holder_dir).await;
    let _ = holder.kill();
    let _ = holder.wait();
    if let Some(pid) = rustix::process::Pid::from_raw(left_pid) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    }
    assert!(table.contains_key(&seen), "the process that left is alive");
    if GROUPS_KNOWN {
        assert_eq!(group, origin(std::process::id()).await, "it kept the group");
    }
    assert!(agent, "a process of an agent passed for the person");
}

/// The runner in a user namespace of its own (a systemd user unit with `ProtectHome=`), and
/// a client outside it: the test that started the runner, a process of the person. The
/// system hides the environment of the client from the runner (DEF-NEW-peerenviron).
#[cfg(target_os = "linux")]
#[test]
fn thr_049_a_client_above_the_namespace_of_the_runner_is_the_person() {
    if !std::process::Command::new("unshare")
        .args(["-U", "true"])
        .status()
        .is_ok_and(|status| status.success())
    {
        eprintln!("skipped: this system makes no user namespace (`unshare -U`)");
        return;
    }
    let exe = std::env::current_exe().expect("the test program");
    let output = std::process::Command::new("unshare")
        .arg("-U")
        .arg(&exe)
        .args(["--ignored", "--exact", "--nocapture"])
        .arg("threads::runner::peer::tests::thr_049_a_runner_in_a_namespace_of_its_own")
        .env(PEER_CLIENT, std::process::id().to_string())
        .output()
        .expect("the runner in a namespace");
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .find(|line| line.starts_with("the client: "))
        .unwrap_or_else(|| panic!("no answer of the runner: {text}"));
    assert!(
        line.ends_with("of an agent false"),
        "the person was taken for an agent: {line}"
    );
}

/// The name of the variable that gives the runner in a namespace the ID of its client.
#[cfg(target_os = "linux")]
const PEER_CLIENT: &str = "PEER_TEST_CLIENT";

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "the runner that the test above starts in a user namespace of its own"]
async fn thr_049_a_runner_in_a_namespace_of_its_own() {
    let client: i32 = std::env::var(PEER_CLIENT)
        .expect("the client")
        .parse()
        .expect("a process ID");
    let hidden = std::fs::read(format!("/proc/{client}/environ")).is_err();
    let agent = of_an_agent(Some(client), Path::new("/no/such/threads")).await;
    println!("the client: environment hidden {hidden}, of an agent {agent}");
}

#[test]
#[ignore = "the process that the test above starts; it only waits"]
fn thr_049_a_process_that_stays() {
    std::thread::sleep(std::time::Duration::from_secs(30));
}
