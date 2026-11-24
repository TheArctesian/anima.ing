use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use axum::{Router, extract::Request, http::StatusCode};

fn git(folder: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .env_clear()
        .env("PATH", GIT_PATH)
        .env("HOME", folder)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(folder)
        .args(args)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    String::from_utf8(result.stdout).unwrap()
}

#[tokio::test]
async fn clone_checks_out_tracked_files_without_installing_hooks_or_submodules() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let staging = temp.path().join("stage");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&staging).unwrap();
    git(&source, &["init", "--quiet", "--template="]);
    std::fs::create_dir(source.join(".git/hooks")).unwrap();
    std::fs::write(
        source.join(".git/hooks/post-checkout"),
        "#!/bin/sh\nexit 1\n",
    )
    .unwrap();
    std::fs::write(source.join("readme"), "clone fixture\n").unwrap();
    std::fs::write(
        source.join(".gitmodules"),
        "[submodule \"component\"]\npath = component\nurl = ../missing\n",
    )
    .unwrap();
    git(&source, &["add", "readme", ".gitmodules"]);
    git(
        &source,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,1111111111111111111111111111111111111111,component",
        ],
    );
    git(
        &source,
        &[
            "-c",
            "user.email=fixture@example.test",
            "-c",
            "user.name=Fixture",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    git(&source, &["branch", "second"]);

    // The local transport is used only by this fixture. Source::checkout allows HTTPS alone.
    checkout(&staging, source.to_str().unwrap(), "protocol.file.allow")
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(staging.join("readme")).unwrap(),
        "clone fixture\n"
    );
    assert!(!staging.join(".git/hooks").exists());
    assert!(!staging.join("component/.git").exists());
    assert_eq!(
        git(&staging, &["rev-parse", "--is-shallow-repository"]).trim(),
        "false"
    );
    git(
        &staging,
        &["show-ref", "--verify", "refs/remotes/origin/second"],
    );
}

#[tokio::test]
async fn dumb_http_is_refused_before_git_requests_objects_or_alternates() {
    let discovery = Arc::new(AtomicBool::new(false));
    let objects = Arc::new(AtomicBool::new(false));
    let server = Router::new().fallback({
        let discovery = Arc::clone(&discovery);
        let objects = Arc::clone(&objects);
        move |request: Request| async move {
            match request.uri().path() {
                "/info/refs" => {
                    discovery.store(true, Ordering::Relaxed);
                    (
                        StatusCode::OK,
                        "1111111111111111111111111111111111111111\trefs/heads/main\n",
                    )
                }
                "/HEAD" => (StatusCode::OK, "ref: refs/heads/main\n"),
                path if path.starts_with("/objects/") => {
                    objects.store(true, Ordering::Relaxed);
                    (StatusCode::NOT_FOUND, "")
                }
                _ => (StatusCode::NOT_FOUND, ""),
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });
    let temp = tempfile::tempdir().unwrap();
    let staging = temp.path().join("stage");
    std::fs::create_dir(&staging).unwrap();

    let result = checkout(
        &staging,
        &format!("http://{address}"),
        "protocol.http.allow",
    )
    .await;
    server.abort();

    assert!(result.is_err());
    assert!(discovery.load(Ordering::Relaxed));
    assert!(!objects.load(Ordering::Relaxed));
}
