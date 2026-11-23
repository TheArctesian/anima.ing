//! One fixed code for each refusal (contract K10 Sections 5 and 7). No refusal holds text of
//! a request, of a file or of a value.

use compose_rules::Rule;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("type_unknown")]
    TypeUnknown,
    #[error("kind_not_permitted")]
    KindNotPermitted,
    #[error("member_unknown")]
    MemberUnknown,
    #[error("secret_unknown")]
    SecretUnknown,
    #[error("applications_off")]
    ApplicationsOff,
    /// The rule of Section 6.4.4 that a file breaks and, for a rule of a service, the
    /// position of the service in the file, from 0.
    #[error("file_not_permitted ({}, service {service:?})", rule.code())]
    FileNotPermitted { rule: Rule, service: Option<usize> },
    #[error("not_adopted")]
    NotAdopted,
    #[error("not_at_machine")]
    NotAtMachine,
    #[error("busy")]
    Busy,
    #[error("nothing_kept")]
    NothingKept,
    #[error("no_digest")]
    NoDigest,
    #[error("name_taken")]
    NameTaken,
    #[error("protected")]
    Protected,
    #[error("start_not_permitted")]
    StartNotPermitted,
    #[error("no_container")]
    NoContainer,
    #[error("program_mismatch")]
    ProgramMismatch,
    #[error("limit_disk")]
    LimitDisk,
}

impl Refusal {
    /// The cause as the contract writes it (K10 Section 5).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TypeUnknown => "type_unknown",
            Self::KindNotPermitted => "kind_not_permitted",
            Self::MemberUnknown => "member_unknown",
            Self::SecretUnknown => "secret_unknown",
            Self::ApplicationsOff => "applications_off",
            Self::FileNotPermitted { .. } => "file_not_permitted",
            Self::NotAdopted => "not_adopted",
            Self::NotAtMachine => "not_at_machine",
            Self::Busy => "busy",
            Self::NothingKept => "nothing_kept",
            Self::NoDigest => "no_digest",
            Self::NameTaken => "name_taken",
            Self::Protected => "protected",
            Self::StartNotPermitted => "start_not_permitted",
            Self::NoContainer => "no_container",
            Self::ProgramMismatch => "program_mismatch",
            Self::LimitDisk => "limit_disk",
        }
    }

    pub(crate) const fn file(rule: Rule) -> Self {
        Self::FileNotPermitted {
            rule,
            service: None,
        }
    }
}

impl From<compose_rules::Refusal> for Refusal {
    /// `not_at_machine` drops the service position: an item's result has no member for it (K10 Section 7).
    fn from(refusal: compose_rules::Refusal) -> Self {
        match refusal {
            compose_rules::Refusal::File { rule, service } => {
                Self::FileNotPermitted { rule, service }
            }
            compose_rules::Refusal::NotAtMachine { .. } => Self::NotAtMachine,
        }
    }
}
