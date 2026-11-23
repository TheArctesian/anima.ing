use std::fmt::Write as _;

use serde_json::Value;

use super::{Change, Verb};
use crate::parts::{CommitSha, Number, RepositoryName, RunId, WorkflowId, is_plain_segment};

/// A path segment written into the provider's code, such as `issues`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixed(&'static str);

impl Fixed {
    /// # Panics
    /// When `segment` is not a plain segment; a `const` makes that fail the build.
    #[must_use]
    pub const fn new(segment: &'static str) -> Self {
        assert!(
            is_plain_segment(segment.as_bytes()),
            "a fixed segment is letters, digits, '.', '-' and '_'"
        );
        Self(segment)
    }
}

/// An absolute path of plain segments, which a URL parser leaves as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangePath(String);

impl ChangePath {
    #[must_use]
    pub fn new(first: Fixed) -> Self {
        Self(String::new()).fixed(first)
    }

    #[must_use]
    pub fn fixed(self, segment: Fixed) -> Self {
        self.then(segment.0)
    }

    /// Two segments: the owner, then the name.
    #[must_use]
    pub fn repository(self, repository: &RepositoryName) -> Self {
        self.then(repository.owner()).then(repository.name())
    }

    #[must_use]
    pub fn number(self, number: Number) -> Self {
        self.then(&number.to_string())
    }

    #[must_use]
    pub fn run(self, run: RunId) -> Self {
        self.then(&run.to_string())
    }

    #[must_use]
    pub fn workflow(self, workflow: &WorkflowId) -> Self {
        self.then(workflow.as_str())
    }

    #[must_use]
    pub fn sha(self, sha: &CommitSha) -> Self {
        self.then(sha.as_str())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn then(mut self, segment: &str) -> Self {
        debug_assert!(
            is_plain_segment(segment.as_bytes()),
            "only checked segments"
        );
        let _ = write!(self.0, "/{segment}");
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    verb: Verb,
    path: ChangePath,
    body: Option<Value>,
}

impl Rendered {
    #[must_use]
    pub const fn new(change: &Change, path: ChangePath, body: Option<Value>) -> Self {
        Self {
            verb: change.verb(),
            path,
            body,
        }
    }

    #[must_use]
    pub const fn verb(&self) -> Verb {
        self.verb
    }

    #[must_use]
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    #[must_use]
    pub const fn body(&self) -> Option<&Value> {
        self.body.as_ref()
    }
}
