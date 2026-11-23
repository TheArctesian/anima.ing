//! The files tunnel (K4 Sections 8.4, 8.5, 9). Each `select!` branch is cancel safe, client
//! sends race the session's end under a time limit, and no lock is held. While a request other
//! than a download's read waits on the device, the client is not read and its silence does not
//! count.

use std::{pin::Pin, sync::Arc, time::Duration};

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures_util::FutureExt as _;
use tokio::{
    sync::watch,
    time::{Instant, sleep_until, timeout},
};

use super::{
    device::{Asked, Broken, Handle, Remote},
    protocol::{self, Listing},
    route::Timing,
    trail::Trail,
    transfer::{Direction, Outcome, Transfer},
};
use crate::fleet::sessions::{
    EndCause, EndReason,
    end::{CLOSE_WAIT, Cause},
    quiet::Quiet,
    upgrade::close,
};

pub(super) enum Done {
    Listed(Asked<Listing>),
    /// The opened file and its size.
    Readable(Asked<(Handle, u64)>),
    Created(Asked<Handle>),
    /// The bytes read and the length asked for.
    Read(Result<Vec<u8>, Broken>, u32),
    Wrote(Result<(), Broken>),
    Closed(Result<(), Broken>),
    Alive(Result<(), Broken>),
}

pub(super) type Work = Pin<Box<dyn Future<Output = Done> + Send>>;

#[derive(Debug)]
pub(super) enum Stage {
    Opening,
    Open(Handle),
    /// The upload was refused; the client's frames, up to its size, are dropped.
    Discarding,
    Closing(Outcome),
}

#[derive(Debug)]
pub(super) struct Active {
    pub(super) transfer: Transfer,
    pub(super) stage: Stage,
}

/// Drops the signal once it resolves, so `endSession` does not wait on this tunnel.
pub(super) struct SessionEnd {
    signal: Option<watch::Receiver<Option<EndCause>>>,
    reason: Option<EndReason>,
}

impl SessionEnd {
    pub(super) const fn new(signal: watch::Receiver<Option<EndCause>>) -> Self {
        Self {
            signal: Some(signal),
            reason: None,
        }
    }

    pub(super) async fn ended(&mut self) -> EndReason {
        if let Some(reason) = self.reason {
            return reason;
        }
        let Some(signal) = self.signal.as_mut() else {
            return std::future::pending().await;
        };
        let reason = loop {
            if let Some(cause) = signal.borrow_and_update().clone() {
                break cause.reason;
            }
            if signal.changed().await.is_err() {
                break EndReason::RemoteClosed;
            }
        };
        self.signal = None;
        self.reason = Some(reason);
        reason
    }
}

enum Turn {
    Ended(EndReason),
    Client(Option<Result<Message, axum::Error>>),
    Done(Done),
    HubSilent,
    ClientIdle,
    WorkStalled,
    DeviceQuiet,
}

pub(super) struct Relay {
    pub(super) client: WebSocket,
    pub(super) remote: Arc<Remote>,
    pub(super) session: SessionEnd,
    pub(super) trail: Trail,
    pub(super) timing: Timing,
    pub(super) max_entries: usize,
    pub(super) work: Option<Work>,
    pub(super) active: Option<Active>,
    pub(super) hub_quiet: Quiet,
    pub(super) client_quiet: Quiet,
    pub(super) work_due: Instant,
    /// An idle link gets a request when this falls due, so it does not end for want of traffic.
    pub(super) device_quiet: Quiet,
}

pub(super) fn after(wait: Duration) -> Instant {
    Instant::now() + wait
}

impl Relay {
    pub(super) async fn run(mut self) {
        let cause = loop {
            let turn = self.next_turn().await;
            if let Err(cause) = self.on_turn(turn).await {
                break cause;
            }
        };
        let work = self.work.take();
        if let Some(active) = self.active.take() {
            let outcome = if cause == Cause::GatewayError {
                Outcome::Failed
            } else {
                Outcome::Cancelled
            };
            self.abandon(active, work, outcome).await;
        }
        close(self.client, None, cause).await;
    }

    /// The client is also read beside a download's read, so `cancel` and `ping` come through.
    fn reads_client(&self) -> bool {
        self.work.is_none()
            || matches!(
                &self.active,
                Some(Active {
                    transfer: Transfer {
                        direction: Direction::Download,
                        ..
                    },
                    stage: Stage::Open(_),
                })
            )
    }

    async fn next_turn(&mut self) -> Turn {
        let reading = self.reads_client();
        let working = self.work.is_some();
        let idle = !working && self.active.is_none();
        tokio::select! {
            reason = self.session.ended() => Turn::Ended(reason),
            message = self.client.recv(), if reading => Turn::Client(message),
            done = finished(&mut self.work), if working => Turn::Done(done),
            () = sleep_until(self.hub_quiet.due()) => Turn::HubSilent,
            () = sleep_until(self.client_quiet.due()), if reading => Turn::ClientIdle,
            () = sleep_until(self.work_due), if working => Turn::WorkStalled,
            () = sleep_until(self.device_quiet.due()), if idle => Turn::DeviceQuiet,
        }
    }

    async fn on_turn(&mut self, turn: Turn) -> Result<(), Cause> {
        match turn {
            Turn::Ended(EndReason::HubShutdown) => Err(Cause::HubShutdown),
            Turn::Ended(_) => Err(Cause::RemoteClosed),
            Turn::Client(message) => self.on_client(message),
            Turn::Done(done) => self.on_done(done).await,
            Turn::HubSilent => self.send_text(protocol::PING).await,
            Turn::ClientIdle => match self.client.recv().now_or_never() {
                Some(message) => self.on_client(message),
                None => Err(Cause::Timeout),
            },
            Turn::WorkStalled => Err(self.broken(Broken::Sftp)),
            Turn::DeviceQuiet => {
                let remote = Arc::clone(&self.remote);
                self.start(async move { Done::Alive(remote.alive().await) });
                Ok(())
            }
        }
    }

    pub(super) fn start(&mut self, work: impl Future<Output = Done> + Send + 'static) {
        self.work = Some(Box::pin(work));
        self.work_due = after(self.timing.device);
    }

    /// The log holds no path, host or device text (LOG-007).
    pub(super) fn broken(&self, broken: Broken) -> Cause {
        let session = self.trail.ticket.session;
        tracing::warn!(%session, error = %broken, "the files tunnel failed");
        Cause::GatewayError
    }

    pub(super) async fn send_text(&mut self, text: &'static str) -> Result<(), Cause> {
        self.send(Message::Text(Utf8Bytes::from_static(text))).await
    }

    pub(super) async fn send(&mut self, message: Message) -> Result<(), Cause> {
        let wait = self.timing.client_idle;
        tokio::select! {
            reason = self.session.ended() => Err(if reason == EndReason::HubShutdown {
                Cause::HubShutdown
            } else {
                Cause::RemoteClosed
            }),
            sent = timeout(wait, self.client.send(message)) => match sent {
                Ok(Ok(())) => {
                    self.hub_quiet.restart(Instant::now());
                    Ok(())
                }
                Ok(Err(_)) => Err(Cause::OperatorClosed),
                Err(_) => Err(Cause::Timeout),
            },
        }
    }

    async fn abandon(&mut self, active: Active, work: Option<Work>, outcome: Outcome) {
        let Active { transfer, stage } = active;
        let audited = !matches!(stage, Stage::Discarding);
        if transfer.direction == Direction::Upload {
            let remote = Arc::clone(&self.remote);
            let _left = timeout(CLOSE_WAIT, leave_upload(&remote, stage, work, &transfer)).await;
        }
        if audited {
            self.audit(&transfer, outcome, None).await;
        }
    }

    pub(super) async fn audit(
        &mut self,
        transfer: &Transfer,
        outcome: Outcome,
        reason: Option<protocol::Refusal>,
    ) -> bool {
        match self.trail.record(transfer, outcome, reason).await {
            Ok(()) => true,
            Err(error) => {
                tracing::error!(%error, "the files tunnel could not record a transfer");
                false
            }
        }
    }
}

/// Only a create that succeeded made a file, and being exclusive that file is the hub's;
/// anything else is left alone.
async fn leave_upload(remote: &Remote, stage: Stage, work: Option<Work>, transfer: &Transfer) {
    let handle = match stage {
        Stage::Open(handle) => Some(handle),
        Stage::Opening => match work {
            Some(work) => match work.await {
                Done::Created(Ok(Ok(handle))) => Some(handle),
                _ => return,
            },
            None => return,
        },
        Stage::Closing(_) => None,
        Stage::Discarding => return,
    };
    if let Some(handle) = &handle {
        let _closed = remote.close(handle).await;
    }
    let _removed = remote.remove(&transfer.path).await;
}

async fn finished(work: &mut Option<Work>) -> Done {
    match work {
        Some(work) => work.as_mut().await,
        None => std::future::pending().await,
    }
}
