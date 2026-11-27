use std::path::{Path, PathBuf};
use std::process::Stdio;

use axum::Router;
use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::response::Response;
use tokio::io::AsyncWriteExt as _;
use tokio::task::JoinHandle;

use super::{GIT_PATH, checkout, git};

pub(super) async fn serve(root: &Path, accept_v2: bool) -> (String, JoinHandle<()>) {
    let router = Router::new()
        .fallback(git_http)
        .with_state((root.to_owned(), accept_v2));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, server)
}

async fn git_http(State((root, accept_v2)): State<(PathBuf, bool)>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let payload = to_bytes(body, 1 << 20).await.unwrap();
    let header = |name| {
        parts
            .headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
    };
    let mut backend = tokio::process::Command::new("git")
        .args(["http-backend"])
        .env_clear()
        .env("PATH", GIT_PATH)
        .env("HOME", &root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_PROJECT_ROOT", &root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("PATH_INFO", parts.uri.path())
        .env("QUERY_STRING", parts.uri.query().unwrap_or_default())
        .env("REQUEST_METHOD", parts.method.as_str())
        .env(
            "HTTP_GIT_PROTOCOL",
            if accept_v2 {
                header("git-protocol")
            } else {
                ""
            },
        )
        .env("CONTENT_TYPE", header("content-type"))
        .env("CONTENT_LENGTH", payload.len().to_string())
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    backend
        .stdin
        .take()
        .unwrap()
        .write_all(&payload)
        .await
        .unwrap();
    let response = backend.wait_with_output().await.unwrap();
    assert!(response.status.success(), "{response:?}");
    cgi_response(&response.stdout)
}

fn cgi_response(bytes: &[u8]) -> Response {
    let split = bytes
        .windows(4)
        .position(|slice| slice == b"\r\n\r\n")
        .unwrap();
    let mut builder = Response::builder();
    for line in std::str::from_utf8(&bytes[..split]).unwrap().lines() {
        let (name, value) = line.split_once(':').unwrap();
        builder = builder.header(name, value.trim());
    }
    builder.body(bytes[split + 4..].to_vec().into()).unwrap()
}

#[tokio::test]
async fn an_empty_smart_repository_can_be_cloned() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    git(root, &["init", "--bare", "--template=", "--quiet", "empty"]);
    for accept_v2 in [true, false] {
        let staging = root.join(format!("stage-v2-{accept_v2}"));
        std::fs::create_dir(&staging).unwrap();
        let (url, server) = serve(root, accept_v2).await;
        checkout(&staging, &format!("{url}/empty"), "protocol.http.allow")
            .await
            .unwrap();
        server.abort();
        assert!(staging.join(".git/HEAD").is_file());
    }
}

#[tokio::test]
async fn a_smart_source_with_shallow_history_is_refused() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    git(root, &["init", "--template=", "--quiet", "source"]);
    let source = root.join("source");
    for commit in ["first", "second", "third"] {
        git(
            &source,
            &[
                "-c",
                "user.email=fixture@example.test",
                "-c",
                "user.name=Fixture",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                commit,
            ],
        );
    }
    git(
        root,
        &[
            "clone",
            "--quiet",
            "--no-local",
            "--depth=1",
            "source",
            "shallow",
        ],
    );
    assert_eq!(
        git(&root.join("shallow"), &["rev-list", "--count", "HEAD"]).trim(),
        "1"
    );
    let staging = root.join("stage");
    std::fs::create_dir(&staging).unwrap();
    let (url, server) = serve(root, true).await;
    let result = checkout(&staging, &format!("{url}/shallow"), "protocol.http.allow").await;
    server.abort();
    assert!(
        result.is_err(),
        "Git accepted the depth-one smart repository"
    );
}
