//! The fixed statements for an application's database and user, given to `psql` on standard input
//! so that no argument holds a password.

use std::fmt;

use super::names::{DatabaseName, Names};

/// 22 letters and digits hold at least 128 bits.
const PASSWORD_LENGTHS: std::ops::RangeInclusive<usize> = 22..=128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    #[error("a password is 22 to 128 letters and digits of ASCII")]
    NotPermitted,
}

/// The password of an application's user, made on the machine (OD-HOST-42 A); only ASCII
/// letters and digits, so SQL, Compose, a connection address and a shell need no quoting (SF-223).
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    /// # Errors
    /// [`PasswordError::NotPermitted`] for a text of another length or with another character.
    pub fn new(text: &str) -> Result<Self, PasswordError> {
        let permitted = PASSWORD_LENGTHS.contains(&text.len())
            && text.bytes().all(|byte| byte.is_ascii_alphanumeric());
        permitted
            .then(|| Self(text.to_owned()))
            .ok_or(PasswordError::NotPermitted)
    }

    pub(super) fn text(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Password(hidden)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Statement {
    text: String,
    secret: bool,
}

impl Statement {
    fn plain(text: String) -> Self {
        Self {
            text,
            secret: false,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for Statement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.secret {
            formatter.write_str("Statement(holds a password)")
        } else {
            write!(formatter, "Statement({:?})", self.text)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    /// Run in the database `postgres`, in this order.
    pub server: Vec<Statement>,
    /// Run in the new database, as the superuser: each extension that the recipe needs.
    pub database: Vec<Statement>,
}

/// A user with no other right, the database that only it reaches, and each extension in it.
/// The caller checks each extension with the recipe (`database_not_described`).
#[must_use]
pub fn setup(names: &Names, password: &Password, extensions: &[String]) -> Setup {
    let user = names.user.quoted();
    let database = names.database.quoted();
    let role = Statement {
        text: format!(
            "CREATE ROLE {user} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT \
             NOREPLICATION NOBYPASSRLS PASSWORD '{}'",
            password.text()
        ),
        secret: true,
    };
    Setup {
        server: vec![
            role,
            new_database(&names.database, &names.user),
            Statement::plain(format!("REVOKE ALL ON DATABASE {database} FROM PUBLIC")),
            Statement::plain(format!(
                "GRANT CONNECT, TEMPORARY ON DATABASE {database} TO {user}"
            )),
        ],
        database: extensions
            .iter()
            .filter(|name| super::is_extension(name))
            .map(|name| Statement::plain(format!("CREATE EXTENSION IF NOT EXISTS \"{name}\"")))
            .collect(),
    }
}

#[must_use]
pub fn new_database(target: &DatabaseName, owner: &DatabaseName) -> Statement {
    Statement::plain(format!(
        "CREATE DATABASE {} OWNER {} ENCODING 'UTF8' TEMPLATE template0",
        target.quoted(),
        owner.quoted()
    ))
}

#[must_use]
pub fn drop_database(target: &DatabaseName) -> Statement {
    Statement::plain(format!(
        "DROP DATABASE IF EXISTS {} WITH (FORCE)",
        target.quoted()
    ))
}

/// The program runs this only with "The hub may delete volumes" set at the machine (R-230).
#[must_use]
pub fn removal(names: &Names) -> Vec<Statement> {
    vec![
        drop_database(&names.database),
        Statement::plain(format!("DROP ROLE IF EXISTS {}", names.user.quoted())),
    ]
}

/// `<database>__back_<number>`; no application name has `__`, so it names no other application.
///
/// # Errors
/// [`super::NameError::NotPermitted`] when the name is longer than 63 bytes.
pub fn way_back_database(names: &Names, number: u32) -> Result<DatabaseName, super::NameError> {
    DatabaseName::new(&format!("{}__back_{number}", names.database))
}
