//! What to tell the operator after one heartbeat (NOT-001, NOT-005, NOT-006); pure, timed by the beat's own `at`.

use chrono::{DateTime, Utc};

use super::model::{CheckState, ExpiryMark, ExpirySubject, NotificationKind, Observation};
use crate::status::Status;

#[cfg(test)]
mod tests;

/// `days` is the whole days left, for the two expiry kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Notice {
    pub kind: NotificationKind,
    pub days: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Decision {
    pub state: CheckState,
    pub notices: Vec<Notice>,
}

/// Maintenance and Unknown change nothing, expiry included; Degraded counts as back up and never sends Down.
pub(super) fn decide(
    o: &Observation,
    state: &CheckState,
    resend_every_beats: u16,
    certificate_days: &[u16],
) -> Decision {
    let mut state = state.clone();
    let mut notices = Vec::new();
    if o.in_maintenance {
        return Decision { state, notices };
    }
    match o.status {
        Status::Maintenance | Status::Unknown => return Decision { state, notices },
        Status::Down => down(&mut state, resend_every_beats, &mut notices),
        Status::Up | Status::Degraded => back_up(o.status, &mut state, &mut notices),
    }
    expiry(o, certificate_days, &mut state.expiry_marks, &mut notices);
    Decision { state, notices }
}

fn plain(kind: NotificationKind) -> Notice {
    Notice { kind, days: None }
}

fn down(state: &mut CheckState, resend_every_beats: u16, notices: &mut Vec<Notice>) {
    if state.notified_status != Some(Status::Down) {
        state.notified_status = Some(Status::Down);
        state.down_beats = 0;
        notices.push(plain(NotificationKind::Down));
        return;
    }
    state.down_beats = state.down_beats.saturating_add(1);
    if resend_every_beats > 0 && state.down_beats >= u32::from(resend_every_beats) {
        state.down_beats = 0;
        notices.push(plain(NotificationKind::StillDown));
    }
}

fn back_up(status: Status, state: &mut CheckState, notices: &mut Vec<Notice>) {
    if state.notified_status == Some(Status::Down) {
        notices.push(plain(NotificationKind::Up));
    }
    state.notified_status = Some(status);
    state.down_beats = 0;
}

/// One notification when any crossed threshold has no mark; a renewal's new `notAfter` drops the subject's old marks.
fn expiry(
    o: &Observation,
    certificate_days: &[u16],
    marks: &mut Vec<ExpiryMark>,
    notices: &mut Vec<Notice>,
) {
    let subjects = [
        (
            ExpirySubject::Certificate,
            o.certificate_not_after,
            NotificationKind::CertificateExpiry,
        ),
        (
            ExpirySubject::Domain,
            o.domain_not_after,
            NotificationKind::DomainExpiry,
        ),
    ];
    for (subject, not_after, kind) in subjects {
        let Some(not_after) = not_after else { continue };
        marks.retain(|m| m.subject != subject || m.not_after == not_after);
        let days_left = whole_days(not_after, o.at);
        let mut fresh = false;
        for threshold_days in certificate_days
            .iter()
            .copied()
            .filter(|t| days_left < u32::from(*t))
        {
            let mark = ExpiryMark {
                subject,
                threshold_days,
                not_after,
            };
            if !marks.contains(&mark) {
                marks.push(mark);
                fresh = true;
            }
        }
        if fresh {
            notices.push(Notice {
                kind,
                days: Some(days_left),
            });
        }
    }
}

fn whole_days(not_after: DateTime<Utc>, at: DateTime<Utc>) -> u32 {
    u32::try_from((not_after - at).num_days()).unwrap_or(0)
}
