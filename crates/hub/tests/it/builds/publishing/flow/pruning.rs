use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::PgPool;

use super::super::{
    fake_r2::Object,
    setup::{File, LINUX_FEED, Publishing, ready, until_waiting, uploaded_linux},
};

fn later(hour: i64) -> DateTime<Utc> {
    ready() + TimeDelta::hours(hour)
}

fn objects(version: &str, files: &[File]) -> Vec<String> {
    let folder = format!("linux-x86_64/{version}");
    files
        .iter()
        .map(|file| format!("{folder}/{}", file.name))
        .chain([format!("{folder}/{}.sig", files[0].name)])
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn bld_086_a_fourth_publish_removes_only_the_oldest_build_files(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let mut releases = Vec::new();
    for (hour, version) in (0..).zip(["0.1.2", "0.1.9", "0.1.10"]) {
        let (_, files) = uploaded_linux(&publishing, version).await;
        assert_eq!(publishing.sweep(later(hour)).await, 1);
        releases.push(objects(version, &files));
    }
    let unrelated = "linux-x86_64/0.1.2/anima-daemon";
    publishing.fake.with(|store| {
        store
            .objects
            .insert(unrelated.to_owned(), Object::new("daemon"));
        assert_eq!(store.deletes, [] as [String; 0]);
    });
    let (latest, files) = uploaded_linux(&publishing, "0.1.11").await;
    releases.push(objects("0.1.11", &files));
    assert_eq!(publishing.sweep(later(3)).await, 1);
    assert_eq!(publishing.job(&latest).await.0, "published");

    for key in &releases[0] {
        assert!(publishing.fake.object(key).is_none(), "{key}");
    }
    for key in releases[1..].iter().flatten() {
        assert!(publishing.fake.object(key).is_some(), "{key}");
    }
    assert!(publishing.fake.object(unrelated).is_some());
    let feed = publishing.fake.object(LINUX_FEED).unwrap();
    let feed: Value = serde_json::from_slice(&feed.bytes).unwrap();
    assert_eq!(feed["version"], "0.1.11");

    let recorded: Vec<(String, String)> = sqlx::query_as(
        "SELECT version, state FROM builds.jobs WHERE objects_removed_at IS NOT NULL",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, [("0.1.2".to_owned(), "published".to_owned())]);
}

#[sqlx::test(migrations = "./migrations")]
async fn bld_086_the_next_publish_retries_a_delete_fault(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let (_, oldest) = uploaded_linux(&publishing, "0.1.1").await;
    assert_eq!(publishing.sweep(later(0)).await, 1);
    for (hour, version) in (1..).zip(["0.1.2", "0.1.3"]) {
        uploaded_linux(&publishing, version).await;
        assert_eq!(publishing.sweep(later(hour)).await, 1);
    }
    let oldest = objects("0.1.1", &oldest);
    let failed = oldest[2].clone();
    publishing
        .fake
        .with(|store| store.reject_delete = Some(failed.clone()));
    let (latest, _) = uploaded_linux(&publishing, "0.1.4").await;
    assert_eq!(publishing.sweep(later(3)).await, 1);
    assert_eq!(publishing.job(&latest).await.0, "published");
    assert!(publishing.fake.object(&failed).is_some());
    assert!(publishing.fake.object(&oldest[0]).is_none());

    publishing.fake.with(|store| store.reject_delete = None);
    uploaded_linux(&publishing, "0.1.5").await;
    assert_eq!(publishing.sweep(later(4)).await, 1);
    assert!(publishing.fake.object(&failed).is_none());
    assert_eq!(
        publishing
            .fake
            .with(|store| store.deletes.iter().filter(|key| **key == failed).count()),
        1
    );
    uploaded_linux(&publishing, "0.1.6").await;
    assert_eq!(publishing.sweep(later(5)).await, 1);
    assert_eq!(
        publishing
            .fake
            .with(|store| store.deletes.iter().filter(|key| **key == failed).count()),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn bld_086_keeps_an_old_version_restored_to_latest_json(pool: PgPool) {
    let publishing = Publishing::start(&pool).await;
    let (_, files) = uploaded_linux(&publishing, "0.1.1").await;
    assert_eq!(publishing.sweep(later(0)).await, 1);
    let rollback = publishing.fake.object(LINUX_FEED).unwrap();
    for (hour, version) in (1..).zip(["0.1.2", "0.1.3"]) {
        uploaded_linux(&publishing, version).await;
        assert_eq!(publishing.sweep(later(hour)).await, 1);
    }
    let (latest, _) = uploaded_linux(&publishing, "0.1.4").await;
    publishing
        .fake
        .with(|store| store.hang_answer = Some(LINUX_FEED.to_owned()));
    let sweep = publishing.sweep(later(3));
    let restore = async {
        until_waiting(&publishing).await;
        publishing.fake.with(|store| {
            store.objects.insert(LINUX_FEED.to_owned(), rollback);
            store.gate.notify_waiters();
        });
    };
    let (ended, ()) = tokio::join!(sweep, restore);
    assert_eq!(ended, 1);
    assert_eq!(publishing.job(&latest).await.0, "published");
    for key in objects("0.1.1", &files) {
        assert!(publishing.fake.object(&key).is_some(), "{key}");
    }
    assert!(publishing.fake.with(|store| store.deletes.is_empty()));
}
