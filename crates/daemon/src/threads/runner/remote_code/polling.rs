use daemon_core::wire::{Claim, Free, RefusalCode, Switch, WorkArchitecture, WorkOs};
use tokio::time::Instant;

use crate::{
    config::home::Home,
    hub::{
        answer::{Answer, Cause},
        remote_code::Claimed,
    },
    remote_code::{
        Store, claim::Advertised, delivery::Delivery, launch::Progress, wire::NotCreated,
    },
    threads::wire::remote_code::{Enrollment, ToHolder},
};

use super::{super::Runner, Event, request_of};

impl Runner {
    pub(in crate::threads::runner) async fn remote_tick(&mut self) {
        let home = self.remote_home();
        if !super::super::serves_a_person(home.dir()) {
            return;
        }
        let Some(config) = home.read_config().ok().flatten() else {
            return;
        };
        let Some(policy) = self.remote_policy(&home) else {
            return;
        };
        if self.holder.fits
            && !self.holder.older
            && self.remote.holder == super::Holder::Unknown
            && self.link.accepts_remote_peer().await
        {
            self.remote.holder = super::Holder::Opening;
            self.remote_send(ToHolder::Open).await;
        }
        if self.remote.holder == super::Holder::Ready {
            self.remote_recover(&home, &config).await;
        }
        self.remote_claim(&home, config.device, &policy);
        self.remote_poll();
        self.remote_controls();
    }

    pub(in crate::threads::runner) async fn remote_event(&mut self, event: Event) {
        match event {
            Event::Claim { connection, result } if connection == self.remote.connection => {
                self.remote.claiming = false;
                if let Some((enrollment, claim)) = result
                    && self.remote.holder == super::Holder::Ready
                    && !self.remote.busy()
                {
                    self.remote.reserving = true;
                    self.remote_send(ToHolder::Reserve { enrollment, claim })
                        .await;
                }
            }
            Event::Poll {
                connection,
                enrollment,
                result,
            } if connection == self.remote.connection => {
                self.remote.polling = false;
                self.remote.delay_poll(&result);
                if let (Some((enrollment, stamp)), Answer::Accepted(commands)) =
                    (enrollment, result)
                    && !commands.is_empty()
                    && self.remote.holder == super::Holder::Ready
                    && enrolled(self.remote_home()).await.is_some_and(|current| {
                        current.config().device == enrollment.device
                            && current.config().hub == enrollment.hub
                            && current.credential_stamp() == stamp
                    })
                {
                    self.remote_commands(commands, &enrollment).await;
                }
                self.remote_poll();
            }
            Event::Relay {
                connection,
                request,
                result,
            } if connection == self.remote.connection && self.remote.active == Some(request) => {
                let frame = match result {
                    Answer::Accepted(reply) => Some(ToHolder::Reply { reply }),
                    Answer::Refused(RefusalCode::WorkOff) => Some(ToHolder::Close {
                        request,
                        cause: NotCreated::Refused,
                    }),
                    Answer::Refused(RefusalCode::NotHolder) => Some(ToHolder::Close {
                        request,
                        cause: NotCreated::Expired,
                    }),
                    _ => Some(ToHolder::Reset { request }),
                };
                if let Some(frame) = frame {
                    self.remote_send(frame).await;
                }
            }
            Event::Relay { .. } | Event::Claim { .. } | Event::Poll { .. } => {}
            Event::Look => self.look_around().await,
            Event::Tick => self.remote_tick().await,
        }
    }

    fn remote_claim(
        &mut self,
        home: &Home,
        device: uuid::Uuid,
        policy: &crate::remote_code::policy::Policy,
    ) {
        if self.remote.claiming || self.remote.busy() {
            return;
        }
        let switch = if policy.starts_enabled() && self.remote.holder == super::Holder::Ready {
            Switch::On
        } else {
            Switch::Off
        };
        let now = Instant::now();
        if self.remote.due_claim.is_some_and(|due| now < due) && self.remote.threads == Some(switch)
        {
            return;
        }
        let Some(sender) = self.remote.events.clone() else {
            return;
        };
        let home = home.clone();
        let targets = self.remote_targets(policy);
        let base = claim(
            device,
            self.may_start(0).is_ok() && self.remote.holder == super::Holder::Ready,
        );
        let cadence = Advertised::new(&base, Switch::Off, switch, &targets).cadence();
        let connection = self.remote.connection;
        self.remote.claiming = true;
        self.remote.threads = Some(switch);
        self.remote.due_claim = Some(now + cadence);
        tokio::spawn(async move {
            let result = if let Some(delivery) = enrolled(home).await {
                let advert = Advertised::new(&base, Switch::Off, switch, &targets);
                match delivery.claim(&advert).await {
                    Answer::Accepted(Claimed::Work(work)) => serde_json::from_value(work)
                        .ok()
                        .map(|claim| (Enrollment::of(delivery.config()), claim)),
                    _ => None,
                }
            } else {
                None
            };
            let _ = sender.send(Event::Claim { connection, result }).await;
        });
    }

    fn remote_poll(&mut self) {
        if self.remote.holder != super::Holder::Ready
            || self.remote.polling
            || self.remote.due_poll.is_some_and(|due| Instant::now() < due)
        {
            return;
        }
        let home = self.remote_home();
        let starts = self
            .remote_policy(&home)
            .is_some_and(|policy| policy.starts_enabled());
        if !starts
            && !self.remote.live.keys().any(|id| {
                self.threads.get(id).is_some_and(|thread| {
                    thread.row.state != agent_threads::control::RunState::Stopped
                })
            })
        {
            return;
        }
        let Some(sender) = self.remote.events.clone() else {
            return;
        };
        let connection = self.remote.connection;
        self.remote.polling = true;
        tokio::spawn(async move {
            let (enrollment, result) = match enrolled(home).await {
                Some(delivery) => (
                    Some((delivery.config().clone(), delivery.credential_stamp())),
                    delivery.poll().await,
                ),
                None => (None, Answer::Failed(Cause::Unreadable)),
            };
            let _ = sender
                .send(Event::Poll {
                    connection,
                    enrollment,
                    result,
                })
                .await;
        });
    }

    async fn remote_recover(&mut self, home: &Home, config: &crate::config::Config) {
        let store = Store::at(home);
        let Ok(rows) = store.enrolled_reservations(config.device, &config.hub) else {
            return;
        };
        for saved in rows {
            let request = request_of(&saved);
            match saved.progress() {
                Progress::Created if self.remote.live.contains_key(&request.runner) => {
                    self.remote_publish(request);
                }
                Progress::Created | Progress::Reconciling
                    if self.threads.contains_key(&request.runner)
                        && self.remote.active != Some(request) =>
                {
                    self.remote_send(ToHolder::Recover { request }).await;
                }
                Progress::Reserved
                    if self.holder.fits && !self.holder.older && !self.remote.busy() =>
                {
                    self.remote_reserved(request).await;
                }
                Progress::Closed(_) => self.remote_evidence(request),
                _ => {}
            }
        }
    }

    fn remote_evidence(&mut self, request: crate::remote_code::wire::DispatchRequest) {
        let now = Instant::now();
        if self
            .remote
            .publications
            .get(&request.runner)
            .is_some_and(|at| *at > now)
        {
            return;
        }
        self.remote
            .publications
            .insert(request.runner, now + std::time::Duration::from_secs(60));
        let home = self.remote_home();
        tokio::spawn(async move {
            if let Some(delivery) = enrolled(home).await {
                let _ = delivery.evidence(&request).await;
            }
        });
    }
}

fn claim(device: uuid::Uuid, free: bool) -> Claim {
    Claim {
        device,
        switch: Switch::Off,
        types: Vec::new(),
        kinds: Vec::new(),
        os: if cfg!(target_os = "macos") {
            WorkOs::Macos
        } else {
            WorkOs::Linux
        },
        architecture: if cfg!(target_arch = "aarch64") {
            WorkArchitecture::Aarch64
        } else {
            WorkArchitecture::X86_64
        },
        free: Free {
            slots: u32::from(free),
            memory_bytes: 0,
            disk_bytes: 0,
        },
        holds: Vec::new(),
        unclean: false,
    }
}

pub(super) async fn enrolled(home: Home) -> Option<Delivery> {
    tokio::task::spawn_blocking(move || Delivery::enrolled(&home))
        .await
        .ok()?
        .ok()
}
