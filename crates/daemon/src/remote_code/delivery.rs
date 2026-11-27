//! The authenticated daemon edge. The holder never receives the bearer.

use std::{collections::BTreeMap, sync::Mutex, time::Duration};

use daemon_core::wire::{RefusalCode, Reported, WorkState};
use serde_json::json;

use crate::{
    config::{
        Config,
        home::{Home, HomeError},
    },
    hub::{
        HubClient, HubError, TIMEOUT,
        answer::{Answer, Cause},
        remote_code::Claimed,
    },
    token_store::{Credential, TokenStore, TokenStoreError},
};

use super::{
    Store,
    broker::{Ask, DispatchReply, Phase, Reply},
    claim::Advertised,
    command::{Delivered, Receipt},
    launch::Reservation,
    wire::{ControlUpload, DispatchRequest, LaunchReceipt},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(35);
const ERROR_DELAY: Duration = Duration::from_secs(5);
static REFUSED: Mutex<BTreeMap<String, [u8; 32]>> = Mutex::new(BTreeMap::new());

/// The two request slots have separate HTTP deadlines. One owns ordinary calls and the other
/// can wait for the full K10 long poll without blocking claim, evidence or result calls.
pub struct Delivery {
    hub: HubClient,
    commands: HubClient,
    config: Config,
    credential: Credential,
    store: Store,
}

#[derive(Debug, thiserror::Error)]
pub enum EnrolledError {
    #[error("the remote Code daemon has no matching enrollment")]
    Enrollment,
    #[error(transparent)]
    Home(#[from] HomeError),
    #[error(transparent)]
    Token(#[from] TokenStoreError),
    #[error(transparent)]
    Hub(#[from] HubError),
}

impl Delivery {
    /// Load one enrollment for both request slots. Reload this session when the daemon
    /// adopts renewed credentials. Token-store I/O belongs on the daemon's blocking lane.
    ///
    /// # Errors
    /// The origin and stored credential disagree, or the enrollment is unavailable.
    pub fn enrolled(home: &Home) -> Result<Self, EnrolledError> {
        let config = home.read_config()?.ok_or(EnrolledError::Enrollment)?;
        let credential = TokenStore::open(home)
            .load()?
            .ok_or(EnrolledError::Enrollment)?;
        if !matches_enrollment(&config, &credential) || refused(&credential) {
            return Err(EnrolledError::Enrollment);
        }
        let own_socket = home.link_socket();
        Ok(Self {
            hub: HubClient::reaching(config.hub.reach(), &own_socket, TIMEOUT)?,
            commands: HubClient::reaching(config.hub.reach(), &own_socket, COMMAND_TIMEOUT)?,
            config,
            credential,
            store: Store::at(home),
        })
    }

    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn credential_stamp(&self) -> [u8; 32] {
        self.credential.token.hash().to_bytes()
    }

    fn observed<T>(&self, answer: Answer<T>) -> Answer<T> {
        if matches!(
            answer,
            Answer::Refused(RefusalCode::TokenRefused | RefusalCode::WrongDevice)
        ) && let Ok(mut refused) = REFUSED.lock()
        {
            refused.insert(
                self.credential.hub.as_text(),
                self.credential.token.hash().to_bytes(),
            );
        }
        answer
    }

    pub async fn claim(&self, claim: &Advertised<'_>) -> Answer<Claimed> {
        if claim.device() != self.config.device {
            return Answer::Refused(RefusalCode::WrongDevice);
        }
        self.observed(self.hub.claim_threads(&self.credential.token, claim).await)
    }

    /// Relay only a request on the enrolled device's active original reservation.
    pub async fn relay(&self, generation: u64, ask: Ask) -> Answer<Reply> {
        let Some(saved) = self.reservation(ask.request()) else {
            return Answer::Failed(Cause::Unreadable);
        };
        if !ask.is_on(generation) || saved.progress() != super::launch::Progress::Reserved {
            return Answer::Failed(Cause::Unreadable);
        }
        let token = &self.credential.token;
        let original = saved.request();
        let answer = match ask.phase() {
            Phase::Prepare => response(self.hub.prepare_thread(token, original).await, |report| {
                ask.reply(DispatchReply::Prepared {
                    stop: report.stop.map(|stop| stop.reason),
                })
            }),
            Phase::Authorize => response(
                self.hub.authorize_thread(token, original).await,
                |decision| ask.reply(DispatchReply::Dispatch { decision }),
            ),
        };
        self.observed(answer)
    }

    fn reservation(&self, request: &DispatchRequest) -> Option<Reservation> {
        let saved = self.store.reservation(request).ok()?;
        saved
            .enrolled_at(self.config.device, &self.config.hub)
            .then_some(saved)
    }

    pub(crate) fn publication(
        &self,
        home: &Home,
        request: &DispatchRequest,
    ) -> Option<super::launch::PublicationProgress> {
        self.reservation(request)?;
        self.store
            .request_publication(request, self.credential_stamp(), || {
                crate::runtime::thread_sync::revision(home)
            })
            .ok()
    }

    pub(crate) async fn remains_enrolled(&self, home: &Home) -> bool {
        let home = home.clone();
        let scope = (
            self.config.device,
            self.config.hub.clone(),
            self.credential_stamp(),
        );
        tokio::task::spawn_blocking(move || {
            let config = home.read_config().ok().flatten();
            let credential = TokenStore::open(&home).load().ok().flatten();
            config.is_some_and(|config| config.device == scope.0 && config.hub == scope.1)
                && credential.is_some_and(|credential| {
                    credential.hub == scope.1
                        && !refused(&credential)
                        && credential.token.hash().to_bytes() == scope.2
                })
        })
        .await
        .unwrap_or(false)
    }

    /// Ordinary reports do not renew or reopen a final attempt. After `NOT_HOLDER` the caller
    /// sends evidence on the original launch tuple; it leaves the local process running.
    pub(crate) async fn report_started(
        &self,
        home: &Home,
        request: &DispatchRequest,
        allowed: impl Fn() -> bool,
    ) -> Answer<Reported> {
        use super::wire::EvidenceOutcome;
        let Some(started) = self.reservation(request).and_then(|saved| saved.evidence()) else {
            return Answer::Failed(Cause::Unreadable);
        };
        let token = &self.credential.token;
        let EvidenceOutcome::Created { thread, .. } = started.outcome else {
            return Answer::Failed(Cause::Unreadable);
        };
        for state in [WorkState::Running, WorkState::Finishing] {
            let report = json!({
                "id": started.work, "attempt": started.attempt, "state": state,
                "log": null, "result": null
            });
            let result = self.observed(self.hub.report_work(token, &report).await);
            if !matches!(result, Answer::Accepted(_)) {
                return result;
            }
        }
        if !self.remains_enrolled(home).await || !allowed() {
            return Answer::Failed(Cause::Unreadable);
        }
        let report = json!({
            "id": started.work, "attempt": started.attempt, "state": WorkState::Done,
            "log": null, "result": {
                "outcome": "succeeded", "cause": null, "seconds": 0, "thread": thread,
                "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" }
            }
        });
        self.observed(self.hub.report_work(token, &report).await)
    }

    pub async fn evidence(&self, request: &DispatchRequest) -> Answer<LaunchReceipt> {
        let Some(evidence) = self.reservation(request).and_then(|saved| saved.evidence()) else {
            return Answer::Failed(Cause::Unreadable);
        };
        self.observed(
            self.hub
                .thread_launch_evidence(&self.credential.token, &evidence)
                .await,
        )
    }

    /// Preserve the dedicated command slot across an empty long poll.
    pub async fn poll(&self) -> Answer<Vec<Delivered>> {
        self.observed(
            self.commands
                .poll_thread_commands(&self.credential.token, self.config.device)
                .await,
        )
    }

    pub async fn receipt(&self, receipt: &Receipt) -> Answer<()> {
        self.observed(
            self.hub
                .report_thread_command(&self.credential.token, receipt)
                .await,
        )
    }

    pub async fn control(
        &self,
        request: &DispatchRequest,
        offer: &super::command::Offer,
    ) -> Answer<()> {
        if self.reservation(request).is_none()
            || !self.store.confirms_offer(request, offer).unwrap_or(false)
        {
            return Answer::Failed(Cause::Unreadable);
        }
        let upload = ControlUpload {
            device: self.config.device,
            launch: request.launch,
            runner: request.runner,
            offer,
        };
        self.observed(
            self.hub
                .upload_thread_control(&self.credential.token, &upload)
                .await,
        )
    }
}

/// A command poll may immediately repeat on success. K8 backoff is supplied by its caller.
#[must_use]
pub fn next_poll<T>(answer: &Answer<T>, backoff: Duration) -> Option<Duration> {
    match answer {
        Answer::Accepted(_) => Some(Duration::ZERO),
        Answer::Refused(RefusalCode::TokenRefused | RefusalCode::WrongDevice) => None,
        Answer::Failed(Cause::Status(429 | 500..=599))
        | Answer::Refused(RefusalCode::Unavailable) => Some(backoff),
        _ => Some(ERROR_DELAY),
    }
}

fn refused(credential: &Credential) -> bool {
    REFUSED.lock().map_or(true, |rejected| {
        rejected.get(&credential.hub.as_text()) == Some(&credential.token.hash().to_bytes())
    })
}

fn matches_enrollment(config: &Config, credential: &Credential) -> bool {
    !config.device.is_nil() && config.hub == credential.hub
}

fn response<A, B>(answer: Answer<A>, read: impl FnOnce(A) -> B) -> Answer<B> {
    match answer {
        Answer::Accepted(value) => Answer::Accepted(read(value)),
        Answer::Refused(code) => Answer::Refused(code),
        Answer::Failed(cause) => Answer::Failed(cause),
    }
}
