use std::io::{BufRead as _, BufReader};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};

use super::*;

#[test]
fn tc7_w3_the_socket_peer_handle_loses_its_liveness_when_the_connector_exits() {
    if Process::open(std::process::id()).is_none() {
        eprintln!("Kernel has no pidfs support; socket proof is unavailable.");
        return;
    }
    let dir = tempfile::tempdir().expect("synthetic socket directory");
    let path = dir.path().join("peer.sock");
    let socket = UnixListener::bind(&path).expect("synthetic listener");
    let script = r#"
        use IO::Socket::UNIX;
        use Socket 'SOCK_STREAM';
        my $socket = IO::Socket::UNIX->new(Type=>SOCK_STREAM, Peer=>$ARGV[0]) or die "connect";
        $socket->autoflush(1);
        print $socket "ready\n";
        <STDIN>;
    "#;
    let mut child = Command::new("/usr/bin/perl")
        .args(["-e", script])
        .arg(&path)
        .env_clear()
        .stdin(Stdio::piped())
        .spawn()
        .expect("synthetic connector");
    let (stream, _) = socket.accept().expect("connected");
    let mut ready = String::new();
    BufReader::new(&stream)
        .read_line(&mut ready)
        .expect("ready");
    assert_eq!(ready, "ready\n");
    let pid = i32::try_from(child.id()).expect("process ID");
    let process = Process::peer(&stream, Some(pid));
    drop(child.stdin.take());
    assert!(child.wait().expect("connector ended").success());
    if let Some(process) = process {
        assert!(
            !process.live(),
            "the original socket peer can no longer deliver"
        );
    } else {
        let (socket, _other) = std::os::unix::net::UnixStream::pair().expect("synthetic socket");
        assert!(
            getsockopt(&socket, PeerPidfd).is_err(),
            "capable sockets must capture the peer"
        );
        eprintln!("Kernel has no SO_PEERPIDFD support; socket proof is unavailable.");
    }
}
