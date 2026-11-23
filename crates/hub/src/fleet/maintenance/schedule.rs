//! When a window is active (MW-001, MW-002, MW-004); the caller passes `now`.
//! A start in a clock-change gap moves to the gap's end, a repeated one to the earlier instant;
//! an occurrence lasts `duration_minutes` of elapsed time.

use chrono::{
    DateTime, Datelike, Days, NaiveDate, NaiveDateTime, TimeDelta, TimeZone, Timelike, Utc,
};
use chrono_tz::Tz;

use super::{
    model::{Weekday, WindowRule},
    rules::DURATION_MAX_MINUTES,
};

const MINUTES_PER_DAY: u32 = 24 * 60;
/// Two days, so a skipped calendar day (Samoa) is stepped over.
const GAP_SEARCH_MINUTES: u32 = 2 * MINUTES_PER_DAY;

/// A manual rule is never active here; `Window::is_active` reads its stored state.
#[must_use]
pub fn is_active(rule: &WindowRule, tz: Tz, now: DateTime<Utc>) -> bool {
    match rule {
        WindowRule::Manual => false,
        WindowRule::Single { starts_at, ends_at } => *starts_at <= now && now < *ends_at,
        _ => rule
            .duration_minutes()
            .is_some_and(|minutes| recurring_is_active(rule, tz, now, minutes)),
    }
}

/// One extra day back because a day with a clock change is shorter.
fn recurring_is_active(rule: &WindowRule, tz: Tz, now: DateTime<Utc>, minutes: u32) -> bool {
    // Bounded like the form's, so a bad row cannot cost a scan of millions of days (MW-004, API-005).
    let minutes = minutes.min(DURATION_MAX_MINUTES);
    let length = TimeDelta::minutes(i64::from(minutes));
    let today = now.with_timezone(&tz).date_naive();
    let days_back = u64::from(minutes.div_ceil(MINUTES_PER_DAY)) + 1;
    (0..=days_back)
        .filter_map(|back| today.checked_sub_days(Days::new(back)))
        .filter_map(|date| occurrence_start(rule, tz, date))
        .any(|start| {
            start <= now
                && start
                    .checked_add_signed(length)
                    .is_some_and(|end| now < end)
        })
}

fn occurrence_start(rule: &WindowRule, tz: Tz, date: NaiveDate) -> Option<DateTime<Utc>> {
    match rule {
        WindowRule::RecurringWeekday {
            weekdays,
            start_time,
            ..
        } => weekdays
            .contains(&Weekday::from(date.weekday()))
            .then(|| local_instant(tz, date.and_time(*start_time))),
        WindowRule::RecurringDayOfMonth {
            days_of_month,
            start_time,
            ..
        } => on_day_of_month(days_of_month, date)
            .then(|| local_instant(tz, date.and_time(*start_time))),
        WindowRule::RecurringInterval {
            first_start_at,
            every_days,
            ..
        } => {
            let first = first_start_at.with_timezone(&tz).naive_local();
            let elapsed = date.signed_duration_since(first.date()).num_days();
            let due = elapsed >= 0 && elapsed.checked_rem(i64::from(*every_days)) == Some(0);
            match (due, elapsed) {
                (false, _) => None,
                // The first occurrence is the exact instant given, even when that local time repeats.
                (true, 0) => Some(*first_start_at),
                (true, _) => Some(local_instant(tz, date.and_time(first.time()))),
            }
        }
        WindowRule::Manual | WindowRule::Single { .. } => None,
    }
}

fn on_day_of_month(days: &[i8], date: NaiveDate) -> bool {
    let is_last = date
        .succ_opt()
        .is_none_or(|next| next.month() != date.month());
    days.iter().any(|day| {
        if *day == -1 {
            is_last
        } else {
            u32::try_from(*day).is_ok_and(|day| day == date.day())
        }
    })
}

fn local_instant(tz: Tz, local: NaiveDateTime) -> DateTime<Utc> {
    tz.from_local_datetime(&local)
        .earliest()
        .map_or_else(|| after_gap(tz, local), |instant| instant.to_utc())
}

/// Gaps begin on a whole minute, so stepping by minutes lands exactly on the change.
fn after_gap(tz: Tz, local: NaiveDateTime) -> DateTime<Utc> {
    let mut probe = local
        .with_second(0)
        .and_then(|time| time.with_nanosecond(0))
        .unwrap_or(local);
    for _ in 0..GAP_SEARCH_MINUTES {
        let Some(next) = probe.checked_add_signed(TimeDelta::minutes(1)) else {
            break;
        };
        probe = next;
        if let Some(instant) = tz.from_local_datetime(&probe).earliest() {
            return instant.to_utc();
        }
    }
    Utc.from_utc_datetime(&local)
}
