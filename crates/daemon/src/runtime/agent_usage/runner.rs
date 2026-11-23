//! The accounts of the programs, asked of this computer's thread runner (THR-128, THR-132).
//! The answer is not logged, and the identifier never leaves the machine (AIS-022, THR-149).

use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::Path;
use std::time::Duration;

use agent_threads::control::{self, Action, Call, Outcome, Reply};
use agent_threads::drive::Program;
use daemon_core::agent_usage::Source;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::UnixStream;

use crate::sampling::agent_usage::ProgramAccount;

/// A runner may take up to 30 s; the batch then goes with no accounts and the next read asks.
const ANSWER_WITHIN: Duration = Duration::from_secs(10);
const MAX_ANSWER: u64 = 64 * 1024;

pub(super) async fn accounts(env: impl Fn(&str) -> Option<String>) -> Option<Vec<ProgramAccount>> {
    let folder = control::folder(env, cfg!(target_os = "macos"))?;
    tokio::time::timeout(ANSWER_WITHIN, ask(&folder.join(control::SOCKET)))
        .await
        .ok()
        .flatten()
}

async fn ask(socket: &Path) -> Option<Vec<ProgramAccount>> {
    let meta = std::fs::symlink_metadata(socket).ok()?;
    let ours = meta.uid() == rustix::process::getuid().as_raw()
        && meta.permissions().mode().trailing_zeros() >= 6
        && meta.file_type().is_socket();
    if !ours {
        return None;
    }
    let stream = UnixStream::connect(socket).await.ok()?;
    let (read, mut write) = stream.into_split();
    let mut call = serde_json::to_string(&Call {
        id: 1,
        action: Action::Accounts,
    })
    .ok()?;
    call.push('\n');
    write.write_all(call.as_bytes()).await.ok()?;
    let mut lines = BufReader::new(tokio::io::AsyncReadExt::take(read, MAX_ANSWER)).lines();
    while let Some(line) = lines.next_line().await.ok()? {
        if let Ok(Reply::Done {
            id: 1,
            result: Outcome::Accounts { accounts },
        }) = serde_json::from_str::<Reply>(&line)
        {
            return Some(
                accounts
                    .into_iter()
                    .map(|row| ProgramAccount {
                        source: source_of(row.program),
                        facts: row.facts,
                    })
                    .collect(),
            );
        }
    }
    None
}

const fn source_of(program: Program) -> Source {
    match program {
        Program::ClaudeCode => Source::ClaudeCode,
        Program::Codex => Source::Codex,
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::UnixListener;

    use super::*;

    fn runner(home: &Path, answer: &'static str, mode: u32) -> tokio::task::JoinHandle<()> {
        let folder = home.join("threads");
        std::fs::create_dir_all(&folder).expect("a folder");
        let socket = folder.join(control::SOCKET);
        let listener = UnixListener::bind(&socket).expect("a socket");
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(mode)).expect("a mode");
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let (read, mut write) = stream.into_split();
            let mut lines = BufReader::new(read).lines();
            let call = lines.next_line().await.expect("a line").expect("a call");
            assert!(call.contains("\"call\":\"accounts\""), "{call}");
            write.write_all(answer.as_bytes()).await.expect("written");
        })
    }

    const ANSWER: &str = concat!(
        r#"{"frame":"done","id":1,"result":{"outcome":"accounts","accounts":["#,
        r#"{"program":"codex","id":"u-1","name":"me@example.com","plan":"pro","limits":[],"at":"2026-11-21T10:00:00Z"}]}}"#,
        "\n"
    );

    #[tokio::test]
    async fn ais_022_the_accounts_come_from_the_runner_socket_of_this_user() {
        let home = tempfile::tempdir().expect("a folder");
        let served = runner(home.path(), ANSWER, 0o600);
        let dir = home.path().to_string_lossy().into_owned();
        let accounts = accounts(|name| (name == "ANIMA_DAEMON_HOME").then(|| dir.clone()))
            .await
            .expect("accounts");
        served.await.expect("served");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].source, Source::Codex);
        assert_eq!(accounts[0].facts.plan.as_deref(), Some("pro"));
    }

    #[tokio::test]
    async fn thr_128_a_socket_that_others_may_use_is_not_asked() {
        let home = tempfile::tempdir().expect("a folder");
        let _served = runner(home.path(), ANSWER, 0o666);
        let dir = home.path().to_string_lossy().into_owned();
        let accounts = accounts(|name| (name == "ANIMA_DAEMON_HOME").then(|| dir.clone())).await;
        assert_eq!(accounts, None);
    }

    #[tokio::test]
    async fn ais_022_no_runner_gives_no_accounts() {
        let home = tempfile::tempdir().expect("a folder");
        let dir = home.path().to_string_lossy().into_owned();
        let accounts = accounts(|name| (name == "ANIMA_DAEMON_HOME").then(|| dir.clone())).await;
        assert_eq!(accounts, None);
    }
}
