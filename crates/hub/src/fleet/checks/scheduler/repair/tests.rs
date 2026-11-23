//! Tests for method repairs (CHK-013, R-064).

macro_rules! repair_test {
    ($(async fn $name:ident($pool:ident: PgPool) $body:block)+) => {$(
        #[sqlx::test(
            migrations = "./migrations",
            fixtures(path = "../../../../../../tests/fixtures", scripts("devices"))
        )]
        async fn $name($pool: sqlx::PgPool) $body
    )+};
}

mod guards;
mod races;
mod rule;
mod support;
mod ticks;
