//! The database fields of an entry (R-216, task H24). The install writes each connection
//! variable, so each is generated.

use crate::{
    Code,
    entry::{Anima, Kind, Need, Server},
    postgresql::is_extension,
};

fn is_variable(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|byte| !byte.is_ascii_digit())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn server_described(server: &Server) -> bool {
    server.major > 0 && server.extensions.iter().all(|name| is_extension(name))
}

fn need_described(need: &Need, generated: &[String]) -> bool {
    let named = need.variables.named();
    let mut unique = named.clone();
    unique.sort_unstable();
    unique.dedup();
    need.lowest_major > 0
        && need.extensions.iter().all(|name| is_extension(name))
        && !named.is_empty()
        && unique.len() == named.len()
        && named
            .iter()
            .all(|name| is_variable(name) && generated.iter().any(|made| made == name))
}

pub(crate) fn check(anima: &Anima) -> Vec<Code> {
    let described = match anima.kind {
        Kind::Database => {
            anima.database.is_none() && anima.server.as_ref().is_some_and(server_described)
        }
        Kind::Application => {
            anima.server.is_none()
                && anima
                    .database
                    .as_ref()
                    .is_none_or(|need| need_described(need, &anima.generated))
        }
    };
    if described {
        Vec::new()
    } else {
        vec![Code::DatabaseNotDescribed]
    }
}
