//! In-crate for `AddressPolicy::allowing_loopback` and the private loop state; nothing reaches the internet.

macro_rules! tick_test {
    ($(async fn $name:ident($pool:ident: PgPool) $body:block)+) => {$(
        #[sqlx::test(
            migrations = "./migrations",
            fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
        )]
        async fn $name($pool: sqlx::PgPool) $body
    )+};
}

mod beaten;
mod decisions;
mod domain;
mod policy;
mod push;
mod races;
mod retries;
mod spawn;
mod status;
mod support;
mod tick;
mod watch;
