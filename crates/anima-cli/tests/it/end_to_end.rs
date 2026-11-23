//! `anima` and `anima mcp` against a real hub per test, on its own database (R-009, R-019).

mod actors;
mod breadth;
mod checks;
mod connect_github;
mod devices;
mod engine_records;
mod keys;
mod mcp;
mod paging;
mod projects;
mod sessions;
mod sign_in;
mod watch;

fn words(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}
