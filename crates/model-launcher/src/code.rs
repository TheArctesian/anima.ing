//! The exit codes of the steps of the launcher (contract K11 Section 5). The launcher writes the
//! code of a failed step to its failure file before it exits, so the daemon can tell an exit of
//! the launcher from an exit of the runtime.

/// A step of the launcher that can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// It could not write itself into the cgroup of the server.
    CgroupJoin,
    /// It could not set the parent-death signal.
    Backstop,
    /// The daemon ended before the parent-death signal was set.
    ParentGone,
    /// The system refused the user and cgroup namespaces.
    Userns,
    /// It could not run the runtime, or its arguments were wrong.
    Exec,
    /// The runtime program is on no valid signed list (R-347).
    NotSigned,
}

impl Step {
    /// Each step, in the order of its code.
    pub const ALL: [Self; 6] = [
        Self::CgroupJoin,
        Self::Backstop,
        Self::ParentGone,
        Self::Userns,
        Self::Exec,
        Self::NotSigned,
    ];

    /// The exit code of a failure at this step.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::CgroupJoin => 120,
            Self::Backstop => 121,
            Self::ParentGone => 122,
            Self::Userns => 123,
            Self::Exec => 124,
            Self::NotSigned => 125,
        }
    }

    /// The step of an exit code.
    #[must_use]
    pub fn of_code(code: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|step| step.code() == code)
    }
}
