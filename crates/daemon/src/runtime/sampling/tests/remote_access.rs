//! Exercises inventory announcements from an isolated Linux installation fixture.

use std::{fs, net::Ipv4Addr, os::unix::fs::PermissionsExt as _, path::Path};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    task::JoinHandle,
};

use daemon_core::wire::BatchInventory;

use super::*;

const SETTINGS: &str = "/etc/anima/remote-access.json";
const PORTS: &str = "/var/lib/anima-link/open-ports";

async fn next_inventory(events: &mut mpsc::Receiver<Event>) -> BatchInventory {
    tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(event) = events.recv().await {
            if let Event::Inventory(inventory) = event {
                return inventory;
            }
        }
        panic!("the inventory probe stopped");
    })
    .await
    .expect("remote access did not appear in a subsequent inventory announcement")
}

async fn serve(port: u16, offer: &'static [u8]) -> JoinHandle<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            stream.write_all(offer).await.unwrap();
            let _ = stream.read_to_end(&mut Vec::new()).await;
        }
    })
}

fn fixture_file(path: &str, contents: &str) {
    let path = Path::new(path);
    assert!(!path.exists());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[tokio::test]
#[ignore = "requires unused AnimaOS paths and loopback ports in a disposable Linux container"]
async fn installed_remote_inventory_follows_listener_recovery_and_owner_changes() {
    fixture_file(
        SETTINGS,
        r#"{"sshUser":"admin","rdpUser":"admin","vncUser":"admin"}"#,
    );
    fixture_file(PORTS, "[3389,5900]");
    let _ssh = serve(22, b"SSH-2.0-test\r\n").await;
    let _rdp = serve(
        3389,
        b"\x03\x00\x00\x13\x0e\xd0\x00\x00\x00\x00\x00\x02\x00\x08\x00\x01\x00\x00\x00",
    )
    .await;
    let insecure_vnc = serve(5900, b"RFB 003.008\n\x01\x01").await;

    let (sender, mut events) = mpsc::channel(CHANNEL_ROOM);
    let probe = tokio::spawn(probe(Platform::Linux, sender));
    let announced = next_inventory(&mut events).await;
    let access = announced.remote_access.unwrap();
    assert_eq!(access.ssh_user, None, "the owner left SSH off");
    assert_eq!(access.rdp_user.as_deref(), Some("admin"));
    assert_eq!(
        announced.remote_vnc_user, None,
        "anonymous VNC must not be announced"
    );

    // Hold the listener unavailable past the original startup-only retry window.
    tokio::time::sleep(Duration::from_secs(65)).await;

    insecure_vnc.abort();
    assert!(insecure_vnc.await.unwrap_err().is_cancelled());
    let _vnc = serve(
        5900,
        b"RFB 003.008\n\x01\x13\x00\x02\x00\x01\x00\x00\x01\x06",
    )
    .await;
    fs::write(PORTS, "[22,5900]").unwrap();
    let announced = next_inventory(&mut events).await;
    let access = announced.remote_access.unwrap();
    assert_eq!(access.ssh_user.as_deref(), Some("admin"));
    assert_eq!(access.rdp_user, None);
    assert_eq!(announced.remote_vnc_user.as_deref(), Some("admin"));

    fs::set_permissions(PORTS, fs::Permissions::from_mode(0o644)).unwrap();
    let announced = next_inventory(&mut events).await;
    let access = announced.remote_access.unwrap();
    assert_eq!(access.ssh_user, None, "the link refuses a shared port file");
    assert_eq!(access.rdp_user, None);
    assert_eq!(announced.remote_vnc_user, None);
    probe.abort();
    assert!(probe.await.unwrap_err().is_cancelled());
    fs::remove_file(PORTS).unwrap();
    fs::remove_file(SETTINGS).unwrap();
}
