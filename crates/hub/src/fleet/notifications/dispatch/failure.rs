//! Why one send failed and whether trying again can help (CR-14 item 5, D-16).
//!
//! A failure is a reason word and, for an HTTP status or an SMTP reply, its code; never the
//! target's text (LOG-007).
//!
//! A host that does not resolve is retried, because a lookup failure is usually brief (R-103);
//! it keeps the reason `TARGET_NOT_PERMITTED` and names no address.

use chrono::{DateTime, Utc};

use crate::{
    fleet::{
        notifications::{Channel, Delivery, DeliveryReason, NotificationKind, egress::SendError},
        targets::TargetError,
    },
    ids::DeliveryId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::notifications) struct Failure {
    pub(in crate::fleet::notifications) reason: DeliveryReason,
    pub(in crate::fleet::notifications) code: Option<i32>,
    unresolved: bool,
}

impl Failure {
    pub(in crate::fleet::notifications) const fn of(reason: DeliveryReason) -> Self {
        Self {
            reason,
            code: None,
            unresolved: false,
        }
    }

    pub(in crate::fleet::notifications) const fn is_permanent(self) -> bool {
        if self.unresolved {
            return false;
        }
        match (self.reason, self.code) {
            (
                DeliveryReason::TargetNotPermitted
                | DeliveryReason::BadAddress
                | DeliveryReason::SecretUnreadable
                | DeliveryReason::SetupFailed,
                _,
            ) => true,
            (DeliveryReason::HttpStatus, Some(status)) => {
                matches!(status, 400..=499) && !matches!(status, 408 | 429)
            }
            (DeliveryReason::SmtpRefused, Some(reply)) => matches!(reply, 500..=599),
            (
                DeliveryReason::HttpStatus
                | DeliveryReason::SmtpRefused
                | DeliveryReason::ConnectionFailed
                | DeliveryReason::TlsFailed
                | DeliveryReason::TimedOut
                | DeliveryReason::NoStarttls
                | DeliveryReason::NotUnderstood,
                _,
            ) => false,
        }
    }
}

impl From<SendError> for Failure {
    fn from(error: SendError) -> Self {
        let (reason, code) = match error {
            SendError::Target(_) => (DeliveryReason::TargetNotPermitted, None),
            SendError::Url => (DeliveryReason::BadAddress, None),
            SendError::Setup => (DeliveryReason::SetupFailed, None),
            SendError::Connect => (DeliveryReason::ConnectionFailed, None),
            SendError::Tls => (DeliveryReason::TlsFailed, None),
            SendError::TimedOut => (DeliveryReason::TimedOut, None),
            SendError::HttpStatus(status) => (DeliveryReason::HttpStatus, Some(i32::from(status))),
            SendError::NoStartTls => (DeliveryReason::NoStarttls, None),
            SendError::SmtpReply(reply) => (DeliveryReason::SmtpRefused, Some(i32::from(reply))),
            SendError::Exchange => (DeliveryReason::NotUnderstood, None),
        };
        let unresolved = matches!(error, SendError::Target(TargetError::Unresolved));
        Self {
            reason,
            code,
            unresolved,
        }
    }
}

/// The entry keeps the channel's name, so it outlives the channel.
pub(in crate::fleet::notifications) fn delivery(
    id: DeliveryId,
    channel: &Channel,
    (kind, at): (NotificationKind, DateTime<Utc>),
    sent: Result<(), Failure>,
) -> Delivery {
    let failure = sent.err();
    Delivery {
        id,
        channel: Some(channel.id),
        channel_name: channel.fields.name.clone(),
        kind,
        at,
        ok: failure.is_none(),
        reason: failure.map(|failure| failure.reason),
        code: failure.and_then(|failure| failure.code),
    }
}
