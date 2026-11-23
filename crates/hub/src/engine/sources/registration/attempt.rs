//! One attempt to connect GitHub (K-E1 Section 7a, PRJ-039).
//! The App's private key lives only in `AppCreated` and `Finishing`, so each step out drops it.

mod shown;
mod steps;

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;
use zeroize::Zeroizing;

pub use self::{
    shown::{App, Failure, Phase, Snapshot},
    steps::{Exchange, Finish, Finished, Refusal, Request, Starter},
};
use super::binding::Hash;
use crate::ids::SourceAccountId;

/// Under the one hour that GitHub's code lives.
pub const LIFETIME: TimeDelta = TimeDelta::minutes(55);
pub const TICKET_LIFETIME: TimeDelta = TimeDelta::minutes(5);
pub const READABLE_FOR: TimeDelta = TimeDelta::minutes(10);
/// A last defence, far past any store that runs.
pub const FINISHING_LIMIT: TimeDelta = TimeDelta::minutes(2);

enum Stage {
    WaitingForBrowser,
    AtGithub {
        exchanging: bool,
    },
    AppCreated {
        key: Zeroizing<String>,
    },
    Finishing {
        key: Zeroizing<String>,
        since: DateTime<Utc>,
        round: u32,
    },
    Connected {
        account: SourceAccountId,
        consumer_on: bool,
    },
    Failed(Failure),
    Cancelled,
}

struct Ticket {
    hash: Hash,
    issued_at: DateTime<Utc>,
}

/// No `Debug` and no `Clone`: it holds the key; what may be shown leaves as a [`Snapshot`].
pub struct Attempt {
    id: Uuid,
    request: Request,
    started_at: DateTime<Utc>,
    stage: Stage,
    ticket: Option<Ticket>,
    binding: Option<Hash>,
    state: Option<Hash>,
    /// S1 once spent: a replayed return from "Create" that presents it is sent to Install again.
    spent: Option<Hash>,
    app: Option<App>,
    rounds: u32,
    ended_at: Option<DateTime<Utc>>,
}

impl Attempt {
    #[must_use]
    pub const fn start(id: Uuid, request: Request, now: DateTime<Utc>) -> Self {
        Self {
            id,
            request,
            started_at: now,
            stage: Stage::WaitingForBrowser,
            ticket: None,
            binding: None,
            state: None,
            spent: None,
            app: None,
            rounds: 0,
            ended_at: None,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }

    #[must_use]
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.started_at + LIFETIME
    }

    /// Whether an App is made and not yet installed: a new start must not replace it.
    #[must_use]
    pub const fn holds_app(&self) -> bool {
        matches!(
            self.stage,
            Stage::AppCreated { .. } | Stage::Finishing { .. }
        )
    }

    #[must_use]
    pub const fn ended(&self) -> bool {
        self.ended_at.is_some()
    }

    #[must_use]
    pub fn readable(&self, now: DateTime<Utc>) -> bool {
        self.ended_at.is_none_or(|ended| now < ended + READABLE_FOR)
    }

    fn end(&mut self, stage: Stage, now: DateTime<Utc>) {
        self.stage = stage;
        self.ended_at = Some(now);
        self.ticket = None;
        self.state = None;
        self.spent = None;
    }

    /// `Finishing` does not expire at `now`: the store's outcome decides.
    pub fn expire(&mut self, now: DateTime<Utc>) -> bool {
        if matches!(self.stage, Stage::Finishing { since, .. } if now >= since + FINISHING_LIMIT) {
            self.back_to_app_created();
        }
        let due = !self.ended() && now >= self.expires_at();
        if due && !matches!(self.stage, Stage::Finishing { .. }) {
            let at = self.expires_at();
            self.end(Stage::Failed(Failure::Expired), at);
            return true;
        }
        false
    }

    const fn waiting_for_browser(&self) -> bool {
        matches!(
            self.stage,
            Stage::WaitingForBrowser | Stage::AtGithub { exchanging: false }
        )
    }

    /// Issues a new ticket with `hash`; an earlier one stops working. Returns when it ends.
    ///
    /// # Errors
    /// `Conflict` outside `WAITING_FOR_BROWSER` and `AT_GITHUB`.
    pub fn issue_ticket(
        &mut self,
        hash: Hash,
        now: DateTime<Utc>,
    ) -> Result<DateTime<Utc>, Refusal> {
        if !self.waiting_for_browser() {
            return Err(Refusal::Conflict);
        }
        self.ticket = Some(Ticket {
            hash,
            issued_at: now,
        });
        Ok(now + TICKET_LIFETIME)
    }

    /// Binds the browser that presented the ticket `presented`, and spends the ticket.
    ///
    /// # Errors
    /// `Ticket` for a wrong, used or old ticket; `Conflict` outside the browser phases.
    pub fn claim_ticket(
        &mut self,
        presented: &str,
        binding: Hash,
        now: DateTime<Utc>,
    ) -> Result<(), Refusal> {
        let valid = self.ticket.as_ref().is_some_and(|ticket| {
            ticket.hash.matches(presented) && now < ticket.issued_at + TICKET_LIFETIME
        });
        if !valid {
            return Err(Refusal::Ticket);
        }
        self.claim(binding)
    }

    /// Binds a signed-in browser with the cookie hash `binding`; one browser holds the attempt.
    ///
    /// # Errors
    /// `Conflict` outside the phases that wait for a browser.
    pub fn claim(&mut self, binding: Hash) -> Result<(), Refusal> {
        if !self.waiting_for_browser() {
            return Err(Refusal::Conflict);
        }
        self.ticket = None;
        self.binding = Some(binding);
        self.state = None;
        self.stage = Stage::AtGithub { exchanging: false };
        Ok(())
    }

    #[must_use]
    pub fn bound(&self, cookie: Option<&str>) -> bool {
        cookie.is_some_and(|cookie| self.binding.is_some_and(|hash| hash.matches(cookie)))
    }

    /// Sets the state `s1` the form to GitHub carries.
    ///
    /// # Errors
    /// `Unbound` for another browser; `Conflict` outside `AT_GITHUB` or during an exchange.
    pub fn send_to_github(&mut self, cookie: Option<&str>, s1: Hash) -> Result<(), Refusal> {
        if !self.bound(cookie) {
            return Err(Refusal::Unbound);
        }
        if !matches!(self.stage, Stage::AtGithub { exchanging: false }) {
            return Err(Refusal::Conflict);
        }
        self.state = Some(s1);
        Ok(())
    }

    /// A fresh match spends the state before the code is converted, so a replay converts nothing.
    ///
    /// # Errors
    /// `Unbound`, `State` or `Conflict`; each leaves the attempt as it was.
    pub fn begin_exchange(
        &mut self,
        cookie: Option<&str>,
        state: Option<&str>,
    ) -> Result<Exchange, Refusal> {
        if !self.bound(cookie) {
            return Err(Refusal::Unbound);
        }
        let matches = |hash: Option<Hash>| {
            hash.zip(state)
                .is_some_and(|(hash, state)| hash.matches(state))
        };
        match self.stage {
            Stage::AppCreated { .. } | Stage::Finishing { .. } if matches(self.spent) => {
                Ok(Exchange::Made)
            }
            Stage::AppCreated { .. } | Stage::Finishing { .. } => Ok(Exchange::Waiting),
            Stage::AtGithub { exchanging: false } => {
                if !matches(self.state) {
                    return Err(Refusal::State);
                }
                self.spent = self.state.take();
                self.stage = Stage::AtGithub { exchanging: true };
                Ok(Exchange::Fresh)
            }
            _ => Err(Refusal::Conflict),
        }
    }

    /// After a cancel or an expiry meanwhile, the App's facts stay so the operator can delete it.
    ///
    /// # Errors
    /// `Conflict` when the attempt no longer waits for this exchange.
    pub fn created(&mut self, app: App, key: Zeroizing<String>, s2: Hash) -> Result<(), Refusal> {
        if !matches!(self.stage, Stage::AtGithub { exchanging: true }) {
            if self.ended() && self.app.is_none() {
                self.app = Some(app);
            }
            return Err(Refusal::Conflict);
        }
        self.app = Some(app);
        self.state = Some(s2);
        self.stage = Stage::AppCreated { key };
        Ok(())
    }

    pub fn exchange_failed(&mut self, failure: Failure, app: Option<App>, now: DateTime<Utc>) {
        if matches!(self.stage, Stage::AtGithub { exchanging: true }) {
            self.app = app;
            self.end(Stage::Failed(failure), now);
        }
    }

    /// GitHub returns `s2` on the Setup URL; a hit without it is not back from Install.
    #[must_use]
    pub fn back_from_install(&self, cookie: Option<&str>, state: Option<&str>) -> bool {
        let state_ok = match self.state {
            Some(hash) => state.is_some_and(|state| hash.matches(state)),
            None => state.is_none(),
        };
        self.bound(cookie) && state_ok
    }

    #[must_use]
    pub fn waiting_app(&self) -> Option<(App, Zeroizing<String>)> {
        match (&self.stage, &self.app) {
            (Stage::AppCreated { key }, Some(app)) => Some((app.clone(), key.clone())),
            _ => None,
        }
    }

    /// `APP_CREATED` becomes `Finishing`, so only one caller stores.
    ///
    /// # Errors
    /// `Conflict` outside `APP_CREATED`, including while another caller stores.
    pub fn begin_finish(&mut self, now: DateTime<Utc>) -> Result<Finish, Refusal> {
        let (Stage::AppCreated { key }, Some(app)) = (&self.stage, &self.app) else {
            return Err(Refusal::Conflict);
        };
        let finish = Finish {
            label: (self.request.label.clone()).unwrap_or_else(|| app.owner.login.clone()),
            app_id: app.id,
            key: key.clone(),
            by: self.request.by.clone(),
            round: self.rounds.wrapping_add(1),
        };
        self.rounds = finish.round;
        let stage = std::mem::replace(&mut self.stage, Stage::Cancelled);
        if let Stage::AppCreated { key } = stage {
            self.stage = Stage::Finishing {
                key,
                since: now,
                round: finish.round,
            };
        }
        Ok(finish)
    }

    fn back_to_app_created(&mut self) {
        let stage = std::mem::replace(&mut self.stage, Stage::Cancelled);
        if let Stage::Finishing { key, .. } = stage {
            self.stage = Stage::AppCreated { key };
        }
    }

    pub fn finished(&mut self, round: u32, outcome: Finished, now: DateTime<Utc>) {
        let current = matches!(self.stage, Stage::Finishing { round: held, .. } if held == round);
        let late_success = matches!(outcome, Finished::Connected { .. }) && self.holds_app();
        if !current && !late_success {
            return;
        }
        match outcome {
            Finished::Connected {
                account,
                consumer_on,
            } => self.end(
                Stage::Connected {
                    account,
                    consumer_on,
                },
                now,
            ),
            Finished::Failed(failure) => self.end(Stage::Failed(failure), now),
            Finished::Again => {
                self.back_to_app_created();
                self.expire(now);
            }
        }
    }

    /// Cancels the attempt and drops any key it holds.
    ///
    /// # Errors
    /// `Conflict` once connected, or while the connection is being stored.
    pub fn cancel(&mut self, now: DateTime<Utc>) -> Result<(), Refusal> {
        match self.stage {
            Stage::Connected { .. } | Stage::Finishing { .. } => Err(Refusal::Conflict),
            Stage::Failed(_) | Stage::Cancelled => Ok(()),
            _ => {
                self.end(Stage::Cancelled, now);
                Ok(())
            }
        }
    }
}
