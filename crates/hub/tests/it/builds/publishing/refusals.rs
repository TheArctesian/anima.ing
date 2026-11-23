//! Publish refusals (BLD-060, BLD-061, BLD-064, BLD-065, BLD-069, BLD-070, D-26, D-27).

use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{
    fake_r2::{KEY_ID, SECRET},
    setup::{
        File, Held, LINUX, LINUX_FEED, LINUX_STABLE, Publishing, linux_feed, linux_files, ready,
        uploaded_linux,
    },
};
use crate::{
    builds::harness::data,
    hub::{error, start},
};

async fn refused(publishing: &Publishing, held: &Held) -> String {
    assert_eq!(publishing.sweep(ready()).await, 1);
    publishing.at(start());
    let (state, reason) = publishing.job(held).await;
    assert_eq!(state, "failed");
    assert!(publishing.fake.object(LINUX_STABLE).is_none());
    assert!(publishing.fake.object(LINUX_FEED).is_none());
    reason.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn bld_064_a_file_read_back_with_another_digest_fails_the_job(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let (held, files) = uploaded_linux(&publishing, "0.1.12").await;
    let tampered = format!("linux-x86_64/0.1.12/{}", files[0].name);
    publishing.fake.with(|store| store.tamper = Some(tampered));
    let reason = refused(&publishing, &held).await;
    assert!(reason.contains("does not have the SHA-256"), "{reason}");
}

#[sqlx::test(migrations = "./migrations")]
async fn bld_064_a_file_read_back_longer_than_reported_fails_the_job(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let (held, _) = uploaded_linux(&publishing, "0.1.12").await;
    let sums = "linux-x86_64/0.1.12/SHA256SUMS".to_owned();
    publishing.fake.with(|store| store.longer = Some(sums));
    let reason = refused(&publishing, &held).await;
    assert!(reason.contains("is not the"), "{reason}");
}

async fn restart(pool: &PgPool) {
    sqlx::query("TRUNCATE builds.releases, builds.builders CASCADE")
        .execute(pool)
        .await
        .unwrap();
}

async fn with_feed(publishing: &Publishing, files: &[File], feed: &Value) -> Held {
    publishing.release("0.1.12", "LINUX_X86_64").await;
    let held = publishing.uploading("pop-os", LINUX).await;
    data(publishing.upload(&held, files, feed).await);
    held
}

#[sqlx::test(migrations = "./migrations")]
async fn r_117_a_proposed_feed_with_a_signature_fails_the_job(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let files = linux_files("0.1.12");
    let mut feed = linux_feed(&publishing, "0.1.12", &files[0]);
    feed["platforms"]["linux-x86_64"]["signature"] = json!("dW50cnVzdGVkIGNvbW1lbnQ6IHg=");
    let held = with_feed(&publishing, &files, &feed).await;
    let reason = refused(&publishing, &held).await;
    assert!(
        reason.contains("the hub signs the updater file"),
        "{reason}"
    );
    let sig = format!("linux-x86_64/0.1.12/{}.sig", files[0].name);
    assert!(publishing.fake.object(&sig).is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn upd_010_sums_that_do_not_list_the_files_fail_the_job(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let mut files = linux_files("0.1.12");
    files[1] = File::new(
        "SHA256SUMS",
        format!("{}  {}\n", "0".repeat(64), files[0].name),
    );
    let feed = linux_feed(&publishing, "0.1.12", &files[0]);
    let held = with_feed(&publishing, &files, &feed).await;
    let reason = refused(&publishing, &held).await;
    assert!(reason.contains("does not list each file"), "{reason}");
}

#[sqlx::test(migrations = "./migrations")]
async fn bld_065_a_feed_with_a_foreign_key_or_host_fails_the_job(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let files = linux_files("0.1.12");
    let good = linux_feed(&publishing, "0.1.12", &files[0]);
    let entry = good["platforms"]["linux-x86_64"].clone();
    let mut foreign_key = good.clone();
    foreign_key["platforms"]["darwin-aarch64"] = entry.clone();
    let mut foreign_host = good.clone();
    foreign_host["platforms"]["linux-x86_64"]["url"] = json!(format!(
        "https://evil.example/linux-x86_64/0.1.12/{}",
        files[0].name
    ));
    for (feed, why) in [
        (foreign_key, "must list exactly"),
        (foreign_host, "needs only a URL"),
    ] {
        restart(&pool).await;
        let held = with_feed(&publishing, &files, &feed).await;
        let reason = refused(&publishing, &held).await;
        assert!(reason.contains(why), "{reason}");
    }
}

/// D-27.
#[sqlx::test(migrations = "./migrations")]
async fn bld_069_a_feed_below_the_published_version_fails_the_job(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    publishing.release("0.1.11", "LINUX_X86_64").await;
    publishing.release("0.1.12", "LINUX_X86_64").await;
    let older = publishing.uploading("pop-os", LINUX).await;
    let newer = publishing.uploading("fedora", LINUX).await;
    let upload = |version: &'static str| {
        let files = linux_files(version);
        let feed = linux_feed(&publishing, version, &files[0]);
        (files, feed)
    };
    let (files, feed) = upload("0.1.12");
    data(publishing.upload(&newer, &files, &feed).await);
    assert_eq!(publishing.sweep(ready()).await, 1);
    let published = publishing.fake.object(LINUX_FEED).unwrap().bytes;
    let (files, feed) = upload("0.1.11");
    publishing.at(start());
    data(publishing.upload(&older, &files, &feed).await);
    assert_eq!(publishing.sweep(ready()).await, 1);
    let (state, reason) = publishing.job(&older).await;
    assert_eq!(state, "failed");
    assert!(reason.unwrap().contains("would go back"));
    assert_eq!(publishing.fake.object(LINUX_FEED).unwrap().bytes, published);
}

#[sqlx::test(migrations = "./migrations")]
async fn d_26_a_report_of_files_other_than_the_grant_is_refused(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    publishing.release("0.1.12", "LINUX_X86_64").await;
    let held = publishing.uploading("pop-os", LINUX).await;
    let files = linux_files("0.1.12");
    data(publishing.grant(&held, &files).await);
    let feed = linux_feed(&publishing, "0.1.12", &files[0]);
    let mut changed = files.clone();
    changed[1].bytes.push(b'\n');
    for reported in [&files[..1], &changed[..]] {
        let response = publishing.report(&held, reported, &feed).await;
        assert_eq!(error(&response).1, "VALIDATION", "{response}");
    }
    assert_eq!(publishing.job(&held).await.0, "uploading");
    assert_eq!(publishing.sweep(ready()).await, 0);
}

fn retargeted(upload: &Value, files: &[File], object: &str) -> Value {
    let url = upload["url"].as_str().unwrap();
    let from = format!("linux-x86_64/0.1.12/{}", files[0].name);
    let mut moved = upload.clone();
    moved["url"] = json!(url.replacen(&from, object, 1));
    moved
}

/// BLD-060, BLD-061, D-28: a presigned URL writes only its own object; the answer holds no token.
#[sqlx::test(migrations = "./migrations")]
async fn bld_061_a_presigned_url_writes_only_its_object_and_length(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    publishing.release("0.1.12", "LINUX_X86_64").await;
    let held = publishing.uploading("pop-os", LINUX).await;
    let files = linux_files("0.1.12");
    let granted = publishing.grant(&held, &files).await;
    let text = granted.to_string();
    assert!(!text.contains(SECRET) && !text.contains("latest.json"));
    assert!(!text.contains("Anima-x86_64.AppImage"));
    let upload = &data(granted)["buildUploadUrls"][0];
    assert_eq!(upload["expiresAt"], "2030-05-05T12:15:00+00:00");
    for object in [
        LINUX_FEED,
        LINUX_STABLE,
        "linux-x86_64/0.1.12/other.AppImage",
    ] {
        let moved = retargeted(upload, &files, object);
        assert_eq!(
            publishing.put(&moved, &files[0].bytes).await,
            403,
            "{object}"
        );
    }
    let mut longer = files[0].bytes.clone();
    longer.push(0);
    assert_eq!(publishing.put(upload, &longer).await, 403);
    let mut retyped = upload.clone();
    retyped["headers"][2]["value"] = json!("text/html");
    assert_eq!(publishing.put(&retyped, &files[0].bytes).await, 403);
    let late = start() + TimeDelta::minutes(15);
    publishing.fake.with(|store| store.now = Some(late));
    assert_eq!(publishing.put(upload, &files[0].bytes).await, 403);
    publishing.fake.with(|store| store.now = Some(start()));
    assert_eq!(publishing.put(upload, &files[0].bytes).await, 200);
    let written = publishing.fake.with(|store| store.writes.clone());
    assert_eq!(written, [format!("linux-x86_64/0.1.12/{}", files[0].name)]);
    assert!(upload["url"].as_str().unwrap().contains(KEY_ID));
}
