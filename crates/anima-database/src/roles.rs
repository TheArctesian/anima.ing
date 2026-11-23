//! The logins and the database of a bundled cluster, as `deploy/postgres/roles.sql` makes them
//! (K2 "Roles", R-016, DB-003).

#[cfg(unix)]
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::PgConnection;
use sqlx::{ConnectOptions as _, Connection as _};

use crate::cluster::SUPERUSER;
use crate::urls::{self, DATABASE, UrlError};
use crate::{Layout, Password, Role};

#[cfg(all(test, unix))]
mod tests;

const RUNTIME_GROUP: &str = "anima_runtime";
/// What `roles.sql` gives a login: no right beyond signing in.
const PLAIN_LOGIN: &str = "login nosuperuser nocreatedb nocreaterole nobypassrls noreplication";

#[derive(Debug, thiserror::Error)]
pub enum RolesError {
    #[error("could not start the database client")]
    Client(#[source] std::io::Error),
    #[error("the superuser could not sign in ({cause})")]
    SignIn { cause: String },
    #[error("could not {step} (SQLSTATE {sqlstate})")]
    Sql {
        step: &'static str,
        sqlstate: String,
    },
    #[error(transparent)]
    Url(#[from] UrlError),
    #[error("the operating system gave no random bytes")]
    Random(#[source] getrandom::Error),
}

/// Makes the logins, the runtime group and the database, then writes the URL files.
/// It is safe to run again after a crash: a login that is there gets a new password.
/// The server gets each login's SCRAM verifier, never a password (LOG-007).
/// It runs its own async runtime, so it is not called from inside one.
///
/// # Errors
/// When the superuser cannot sign in, a statement fails or a URL file cannot be written.
pub fn prepare(layout: &Layout) -> Result<(), RolesError> {
    let owner = Fresh::new()?;
    let hub = Fresh::new()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(RolesError::Client)?
        .block_on(apply(layout, &owner.verifier(), &hub.verifier()))?;
    urls::write(layout, Role::Owner, &owner.password)?;
    urls::write(layout, Role::Hub, &hub.password)?;
    Ok(())
}

struct Fresh {
    password: Password,
    salt: [u8; 16],
}

impl Fresh {
    fn new() -> Result<Self, RolesError> {
        let mut salt = [0_u8; 16];
        getrandom::fill(&mut salt).map_err(RolesError::Random)?;
        Ok(Self {
            password: Password::random().map_err(RolesError::Random)?,
            salt,
        })
    }

    fn verifier(&self) -> String {
        self.password.scram_verifier(&self.salt)
    }
}

/// Keeps each statement and its parse tree out of the server log. The session sets them again
/// after the caller's `PGOPTIONS`, so this list has the last word.
const QUIET: [(&str, &str); 11] = [
    ("log_statement", "none"),
    ("log_min_error_statement", "panic"),
    ("log_min_duration_statement", "-1"),
    ("log_min_duration_sample", "-1"),
    ("log_transaction_sample_rate", "0"),
    ("log_min_messages", "log"),
    ("log_error_verbosity", "terse"),
    ("debug_print_parse", "off"),
    ("debug_print_rewritten", "off"),
    ("debug_print_plan", "off"),
    ("client_min_messages", "notice"),
];

async fn apply(
    layout: &Layout,
    owner_verifier: &str,
    hub_verifier: &str,
) -> Result<(), RolesError> {
    #[cfg(unix)]
    let options = PgConnectOptions::new_without_pgpass()
        .socket(layout.socket_folder())
        .port(5432);
    #[cfg(windows)]
    let endpoint = crate::windows::Endpoint::load(layout).map_err(RolesError::Client)?;
    #[cfg(windows)]
    let password = endpoint.password().map_err(RolesError::Client)?;
    #[cfg(windows)]
    let options = endpoint.options().password(password.expose());
    let mut session = options
        .username(SUPERUSER)
        .database("postgres")
        .application_name("anima-database")
        .options(QUIET)
        .disable_statement_logging()
        .connect()
        .await
        .map_err(|error| RolesError::SignIn {
            cause: sqlstate(&error),
        })?;
    for (setting, value) in QUIET {
        let statement = format!("set {setting} = {value}");
        run(&mut session, "set the session's logging off", &statement).await?;
    }
    login_group(&mut session).await?;
    let question = "select exists (select from pg_catalog.pg_roles where rolname = $1)";
    for (role, group, verifier) in [
        (Role::Owner, "", owner_verifier),
        (Role::Hub, RUNTIME_GROUP, hub_verifier),
    ] {
        let is_there = exists(&mut session, "make a login", question, role.login()).await?;
        let statement = login_statement(role.login(), is_there, group, verifier);
        run(&mut session, "make a login", &statement).await?;
    }
    database(&mut session).await?;
    session.close().await.ok();
    Ok(())
}

/// A verifier is Base64 with `$` and `:`, so it needs no quoting.
fn login_statement(name: &str, is_there: bool, group: &str, verifier: &str) -> String {
    if is_there {
        return format!("alter role {name} {PLAIN_LOGIN} password '{verifier}'");
    }
    let membership = if group.is_empty() {
        String::new()
    } else {
        format!("in role {group} ")
    };
    format!("create role {name} {PLAIN_LOGIN} {membership}password '{verifier}'")
}

async fn exists(
    session: &mut PgConnection,
    step: &'static str,
    question: &'static str,
    name: &str,
) -> Result<bool, RolesError> {
    sqlx::query_scalar(question)
        .bind(name)
        .fetch_one(session)
        .await
        .map_err(|error| sql_error(step, &error))
}

async fn login_group(session: &mut PgConnection) -> Result<(), RolesError> {
    const STEP: &str = "make the runtime group";
    let question = "select exists (select from pg_catalog.pg_roles where rolname = $1)";
    if !exists(session, STEP, question, RUNTIME_GROUP).await? {
        run(
            session,
            STEP,
            &format!("create role {RUNTIME_GROUP} nologin"),
        )
        .await?;
    }
    Ok(())
}

async fn database(session: &mut PgConnection) -> Result<(), RolesError> {
    const STEP: &str = "make the database";
    let owner = Role::Owner.login();
    let question = "select exists (select from pg_catalog.pg_database where datname = $1)";
    if !exists(session, STEP, question, DATABASE).await? {
        run(
            session,
            STEP,
            &format!("create database {DATABASE} owner {owner}"),
        )
        .await?;
    }
    for grant in [
        // Also when the database was there: only the owner's database takes migrations.
        format!("alter database {DATABASE} owner to {owner}"),
        format!("revoke connect, temporary on database {DATABASE} from public"),
        format!("grant connect on database {DATABASE} to {RUNTIME_GROUP}"),
        "revoke connect on database postgres from public".to_owned(),
        "revoke connect on database template1 from public".to_owned(),
    ] {
        run(session, "set who may connect", &grant).await?;
    }
    Ok(())
}

async fn run(
    session: &mut PgConnection,
    step: &'static str,
    statement: &str,
) -> Result<(), RolesError> {
    sqlx::raw_sql(statement)
        .execute(session)
        .await
        .map(|_| ())
        .map_err(|error| sql_error(step, &error))
}

fn sql_error(step: &'static str, error: &sqlx::Error) -> RolesError {
    RolesError::Sql {
        step,
        sqlstate: sqlstate(error),
    }
}

/// Never the message: the server can quote the statement in it.
fn sqlstate(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(database) => database
            .code()
            .map_or_else(|| "none".to_owned(), std::borrow::Cow::into_owned),
        sqlx::Error::Io(source) => format!("no answer: {}", source.kind()),
        _ => "no answer".to_owned(),
    }
}
