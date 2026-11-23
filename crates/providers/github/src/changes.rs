//! The named changes GitHub can make (OD-E14, PRJ-027 to PRJ-035). Pure: the engine sends them.

mod render;
mod settle;

pub(crate) use render::render;
pub(crate) use settle::settle;

#[cfg(test)]
mod tests;
