//! `anima` run as a program against a scripted hub on a loopback port. The tests keep tokens in
//! the 0600 token file (`ANIMA_TOKEN_STORE=file`), never in this computer's key store.

mod basics;
mod catalogue;
mod containers;
mod coverage;
mod end_to_end;
mod fleet;
mod hub_records;
mod kit;
mod mcp;
