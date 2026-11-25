//! One run of `git` as its caller describes it: where, for whom, with which arguments.
//!
//! Pure: a value and its builders.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

use super::{Asked, LOCAL_TIME};

/// One run: where, for whom, with which arguments.
pub struct Run<'a> {
    pub dir: &'a Path,
    pub asked: Asked,
    pub args: Vec<OsString>,
    /// More environment for this run alone (a private index file).
    pub env: Vec<(&'static str, OsString)>,
    /// More configuration for this run alone, given through the environment: a value here is
    /// in no list of arguments, which each user of the computer can read.
    pub settings: Vec<(String, String)>,
    pub input: Option<Vec<u8>>,
    pub time: Duration,
    pub(super) output_prefix: Option<u64>,
}

impl<'a> Run<'a> {
    /// A run of the runner itself in `dir`.
    pub fn new<S: AsRef<OsStr>>(dir: &'a Path, args: impl IntoIterator<Item = S>) -> Self {
        Self {
            dir,
            asked: Asked::Runner,
            args: args
                .into_iter()
                .map(|arg| arg.as_ref().to_owned())
                .collect(),
            env: Vec::new(),
            settings: Vec::new(),
            input: None,
            time: LOCAL_TIME,
            output_prefix: None,
        }
    }

    #[must_use]
    pub fn for_the_person(mut self) -> Self {
        self.asked = Asked::Person;
        self
    }

    #[must_use]
    pub fn with_env(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.env.push((name, value.into()));
        self
    }

    #[must_use]
    pub fn with_setting(mut self, name: &str, value: &str) -> Self {
        self.settings.push((name.to_owned(), value.to_owned()));
        self
    }

    #[must_use]
    pub fn with_input(mut self, input: Vec<u8>) -> Self {
        self.input = Some(input);
        self
    }

    #[must_use]
    pub(in crate::threads::git) const fn with_output_prefix(mut self, bytes: u64) -> Self {
        self.output_prefix = Some(bytes);
        self
    }

    #[must_use]
    pub const fn with_time(mut self, time: Duration) -> Self {
        self.time = time;
        self
    }
}
