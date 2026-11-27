//! One-time approval to activate the exact staged release within an owner-selected window.
use crate::{Error, Result, state, update};
use anima_os_maintenance::{
    Admission, Lease, Operation,
    recovery::boot_id,
    update_window::{DrainScope, Phase, RECEIPT, Window, now},
};
use serde_json::{Value, json};
use std::path::Path;
use uuid::Uuid;

fn phase_at(window: &Window, observed_at: u64, review: &str) -> Phase {
    if !window.phase.pending() {
        window.phase
    } else if observed_at >= window.ends_at {
        Phase::Expired
    } else if window.review != review {
        Phase::Changed
    } else {
        window.phase
    }
}

fn save(window: &Window) -> Result<()> {
    state::atomic(Path::new(RECEIPT), &serde_json::to_vec(window)?)
}

fn view(window: &Window, phase: Phase) -> Value {
    let reason = if phase == window.phase {
        window.reason.as_deref()
    } else {
        None
    };
    json!({
        "id": window.id, "version": window.version, "startsAt": window.starts_at,
        "endsAt": window.ends_at, "phase": phase, "reason": reason,
        "pausesNewWork": window.drain.is_some() && phase.pending(),
    })
}

pub fn status(review: &str) -> Result<Value> {
    match Window::read()? {
        Some(window) => Ok(view(&window, phase_at(&window, now()?, review))),
        None => Ok(Value::Null),
    }
}

pub async fn dispatch(args: &[String]) -> Option<Result<Value>> {
    Some(match args {
        [action, review, start, end] if action == "schedule-activation" => {
            schedule(review, start, end)
        }
        [action, id] if action == "cancel-activation" => cancel(id),
        [action] if action == "attempt-window" => attempt().await,
        _ => return None,
    })
}

fn schedule(review: &str, start: &str, end: &str) -> Result<Value> {
    state::prepare()?;
    let _admission = Admission::configuration()?;
    let _lock = state::writer()?;
    state::require_review("activate", review)?;
    let staged = state::read()?;
    let version = staged["version"]
        .as_str()
        .filter(|_| staged["phase"] == "staged")
        .ok_or(Error::Refused("Download and verify an update first."))?;
    let now = now()?;
    let (starts_at, ends_at) = times(now, start, end)?;
    if let Some(current) = Window::read()?
        && phase_at(&current, now, review).pending()
    {
        if (current.starts_at, current.ends_at) == (starts_at, ends_at) {
            return Ok(view(&current, current.phase));
        }
        return Err(Error::Refused(
            "Cancel the current activation window before scheduling another.",
        ));
    }
    let window = Window {
        id: Uuid::now_v7(),
        version: version.to_owned(),
        review: review.to_owned(),
        starts_at,
        ends_at,
        phase: Phase::Scheduled,
        reason: None,
        drain: Some(DrainScope {
            boot: boot_id()?,
            system: update::system_of(&staged, "system")?.to_owned(),
            previous: update::system_of(&staged, "previous")?.to_owned(),
        }),
    };
    save(&window)?;
    Ok(view(&window, window.phase))
}

fn times(now: u64, start: &str, end: &str) -> Result<(u64, u64)> {
    let times = start.parse::<u64>().ok().zip(end.parse::<u64>().ok());
    times
        .filter(|&(start, end)| {
            start >= now
                && start - now <= 30 * 86_400
                && (120..=86_400).contains(&end.saturating_sub(start))
        })
        .ok_or(Error::Refused(
            "Choose a future window within 30 days that lasts between 2 minutes and 24 hours.",
        ))
}

fn cancel(id: &str) -> Result<Value> {
    let id =
        Uuid::parse_str(id).map_err(|_| Error::Refused("Choose the current activation window."))?;
    state::prepare()?;
    let _admission = Admission::cleanup()?;
    let _lock = state::writer()?;
    let mut window = Window::read()?
        .filter(|window| window.id == id)
        .ok_or(Error::Refused(
            "The activation window changed. Review it again.",
        ))?;
    if window.phase.pending() {
        window.phase = Phase::Cancelled;
        window.reason = None;
        save(&window)?;
    } else if window.phase != Phase::Cancelled {
        return Err(Error::Refused(
            "The activation window is no longer scheduled.",
        ));
    }
    Ok(view(&window, window.phase))
}

async fn attempt() -> Result<Value> {
    state::prepare()?;
    let Some(window) = Window::read()?.filter(|window| window.phase.pending()) else {
        return Ok(Value::Null);
    };
    let observed_at = now()?;
    let status = phase_at(&window, observed_at, &state::activation_revision()?);
    if status.pending() && observed_at < window.starts_at {
        return Ok(view(&window, status));
    }
    // Take the maintenance owner before the updater lock, matching manual activation.
    let lease = if status.pending() {
        Some(Lease::prepare(Operation::Activate).await)
    } else {
        None
    };
    let _lock = state::writer()?;
    let Some(mut current) =
        Window::read()?.filter(|current| current.id == window.id && current.phase.pending())
    else {
        return Ok(Value::Null);
    };
    let observed_at = now()?;
    current.phase = if status.pending() {
        phase_at(&current, observed_at, &state::activation_revision()?)
    } else {
        status
    };
    if !current.phase.pending() {
        current.reason = None;
        save(&current)?;
    } else if observed_at >= current.starts_at
        && let Some(lease) = lease
    {
        match lease {
            Ok(lease) => activate(&mut current, lease).await?,
            Err(error) => {
                let reason = error.to_string();
                if current.phase != Phase::Deferred || current.reason.as_deref() != Some(&reason) {
                    current.phase = Phase::Deferred;
                    current.reason = Some(reason);
                    save(&current)?;
                }
            }
        }
    }
    Ok(view(&current, current.phase))
}

async fn activate(window: &mut Window, lease: Lease) -> Result<()> {
    let pending = update::activation_target();
    let observed_at = now()?;
    if observed_at < window.starts_at {
        return Ok(());
    }
    window.reason = None;
    if observed_at >= window.ends_at {
        window.phase = Phase::Expired;
        return save(window);
    }
    let result = match pending {
        Ok(pending) => {
            window.phase = Phase::Dispatching;
            save(window)?;
            update::activate(pending, lease).await.map(drop)
        }
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        window.phase = Phase::NeedsReview;
        window.reason = Some(error.to_string());
        save(window)?;
    }
    Ok(())
}
