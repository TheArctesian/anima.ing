//! A refused builder call leaves the job's row unlocked for the next claim or publisher take
//! (D-43, D-25). A dropped transaction rolls back only once this single-thread runtime runs a
//! task, so the lock check from another thread does not depend on load.

use chrono::{DateTime, Utc};
use sqlx::{Connection as _, PgConnection, PgPool, postgres::PgConnectOptions};

use super::{BUILDER, JOB, RELEASE, builder};
use crate::fleet::{
    builds::{
        BuildQueue, Error, Updates, jobs,
        model::{Report, UploadedFile},
    },
    events::Bus,
};

const OPERATOR: &str = "0190a000-0000-7000-8009-0000000000f1";

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_900_000_000, 0).unwrap()
}

/// No builder holds the job, so every builder call on it is refused after the lock.
pub(in crate::fleet::builds) async fn queued_job(pool: &PgPool, job: &str) {
    sqlx::query(
        "insert into access.operators (id, username, password_hash, created_at) \
         values ($1::uuid, 'operator', '$argon2id$x', $2)",
    )
    .bind(OPERATOR)
    .bind(now())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into builds.releases (id, version, commit_sha, created_by, created_by_name, \
                created_at) \
         values ($1::uuid, '0.1.12', repeat('a', 40), $2::uuid, 'operator', $3)",
    )
    .bind(RELEASE)
    .bind(OPERATOR)
    .bind(now())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into builds.jobs (id, release_id, target, version, state, created_at, \
                queued_at, state_changed_at) \
         values ($1::uuid, $2::uuid, 'linux-x86_64', '0.1.12', 'queued', $3, $3, $3)",
    )
    .bind(job)
    .bind(RELEASE)
    .bind(now())
    .execute(pool)
    .await
    .unwrap();
}

pub(in crate::fleet::builds) fn row_locked(options: PgConnectOptions, job: &str) -> bool {
    let job = job.to_owned();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut conn = PgConnection::connect_with(&options).await.unwrap();
            let asked =
                sqlx::query("select 1 from builds.jobs where id = $1::uuid for update nowait")
                    .bind(job)
                    .execute(&mut conn)
                    .await;
            match asked {
                Ok(_) => false,
                Err(error) => {
                    let code = error
                        .as_database_error()
                        .and_then(sqlx::error::DatabaseError::code);
                    assert_eq!(code.as_deref(), Some("55P03"), "{error}");
                    true
                }
            }
        })
    })
    .join()
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn d_43_a_refused_report_leaves_the_job_unlocked(pool: PgPool) {
    queued_job(&pool, JOB).await;
    let (bus, updates) = (Bus::new(16), Updates::new());
    let build_queue = BuildQueue {
        db: &pool,
        bus: &bus,
        updates: &updates,
    };
    let report = Report {
        job: JOB.parse().unwrap(),
        state: None,
        reason: None,
        lines: vec!["a line".to_owned()],
    };
    let refused = jobs::report(build_queue, &builder(BUILDER), report, now()).await;
    assert!(matches!(refused, Err(Error::Forbidden(_))), "{refused:?}");
    assert!(!row_locked((*pool.connect_options()).clone(), JOB));
}

#[sqlx::test(migrations = "./migrations")]
async fn d_43_a_refused_uploaded_report_leaves_the_job_unlocked(pool: PgPool) {
    queued_job(&pool, JOB).await;
    let (bus, updates) = (Bus::new(16), Updates::new());
    let build_queue = BuildQueue {
        db: &pool,
        bus: &bus,
        updates: &updates,
    };
    let uploaded = jobs::Upload {
        job: JOB.parse().unwrap(),
        files: vec![UploadedFile {
            name: "SHA256SUMS".to_owned(),
            sha256: "0".repeat(64),
            size: 1,
        }],
        feed: "{}".to_owned(),
    };
    let refused = jobs::report_uploaded(build_queue, &builder(BUILDER), uploaded, now()).await;
    assert!(matches!(refused, Err(Error::Forbidden(_))), "{refused:?}");
    assert!(!row_locked((*pool.connect_options()).clone(), JOB));
}

fn transaction_left_open(options: PgConnectOptions) -> bool {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut conn = PgConnection::connect_with(&options).await.unwrap();
            let open: i64 = sqlx::query_scalar(
                "select count(*) from pg_stat_activity \
                 where datname = current_database() and pid <> pg_backend_pid() \
                   and state like 'idle in transaction%'",
            )
            .fetch_one(&mut conn)
            .await
            .unwrap();
            open > 0
        })
    })
    .join()
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn d_43_a_claim_refused_as_not_enrolled_leaves_no_transaction_open(pool: PgPool) {
    queued_job(&pool, JOB).await;
    let (bus, updates) = (Bus::new(16), Updates::new());
    let build_queue = BuildQueue {
        db: &pool,
        bus: &bus,
        updates: &updates,
    };
    let refused = jobs::claim(build_queue, &builder(BUILDER), now()).await;
    assert!(matches!(refused, Err(Error::Forbidden(_))), "{refused:?}");
    assert!(!transaction_left_open((*pool.connect_options()).clone()));
}
