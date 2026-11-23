//! The PostgreSQL entry as pure functions; the applications program runs what they give (R-216, OD-HOST-42 A).
//!
//! No statement holds hub text: each identifier is a checked, quoted name, each extension is
//! of a fixed list, and the password is a quoted literal of letters and digits.

mod connection;
mod copies;
mod names;
mod statements;
#[cfg(test)]
mod tests;

pub use connection::{Address, Value, values};
pub use copies::{
    DumpFile, Restore, check_command, dump_command, list_command, missing_tables, restore_command,
    tables_in_list, tables_query, way_back, way_back_test,
};
pub use names::{DatabaseName, NameError, Names, is_extension};
pub use statements::{
    Password, PasswordError, Setup, Statement, drop_database, new_database, removal, setup,
    way_back_database,
};
