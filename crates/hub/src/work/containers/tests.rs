//! Tests of the containers of a device, in-crate for the loopback policy. The queries themselves
//! are tested in the integration crate.

mod detail;
mod docker;
mod egress;
mod names;
mod project;
mod reads;
mod registry;
mod sentences;
mod storage;
mod updates;

use sqlx::PgPool;

use super::Reach;
use crate::fleet::ssh::tests::{Answer, Bench, Heard, bench, reaching};

pub(super) const ID: &str = "4f5c1d2e3a4b5c6d7e8f90123456789abcdef0123456789abcdef0123456789a";
/// Nothing the hub writes besides the answer may hold this text of a container's log.
const SECRET: &str = "API_TOKEN=hunter2";

pub(super) const LISTED: &str = concat!(
    r#"{"id":"4f5c1d2e3a4b5c6d7e8f90123456789abcdef0123456789abcdef0123456789a","#,
    r#""names":"dawarich_app","image":"freikin/dawarich:latest","state":"running","#,
    r#""status":"Up 3 hours (healthy)","ports":"0.0.0.0:3000->3000/tcp, [::]:3000->3000/tcp","#,
    r#""created":"2026-11-18 10:00:00 -0800 PST","project":"dawarich","service":"app","#,
    r#""dir":"/srv/dawarich","files":"/srv/dawarich/compose.yaml,/srv/dawarich/override.yaml"}"#,
    "\n",
    r#"{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","#,
    r#""names":"web/db,old-db","image":"postgres:17","state":"exited","#,
    r#""status":"Exited (0) 2 days ago","ports":"","created":"not a time","#,
    r#""project":"","service":""}"#,
    "\n",
);

pub(super) fn exiting(stdout: &str, status: u32) -> Answer {
    Answer::Exit {
        stdout: stdout.as_bytes().to_vec(),
        stderr: SECRET.as_bytes().to_vec(),
        status,
    }
}

pub(super) async fn device(pool: &PgPool, answer: Answer) -> Bench {
    bench(pool, answer).await
}

fn reach<'a>(bench: &'a Bench, policy: &'a crate::fleet::targets::AddressPolicy) -> Reach<'a> {
    Reach {
        db: &bench.db,
        vault: &bench.vault,
        policy,
    }
}

fn execs(bench: &Bench) -> Vec<String> {
    bench
        .server
        .heard()
        .into_iter()
        .filter_map(|heard| match heard {
            Heard::Exec(line) => Some(line),
            _ => None,
        })
        .collect()
}

pub(super) fn policy() -> crate::fleet::targets::AddressPolicy {
    reaching()
}
