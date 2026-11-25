use agent_threads::{
    control::{Change, RunState},
    drive::Mode,
};

use crate::{
    config::home::Home,
    hub::answer::Answer,
    remote_code::{
        Store,
        delivery::Delivery,
        launch::Progress,
        policy::Policy,
        wire::{DispatchRequest, NotCreated},
    },
    threads::wire::{
        Action,
        remote_code::{Start, ToHolder, ToRunner},
    },
};

use super::{
    super::{Runner, calls::codex_of, login, start::NewThread, thread::Thread},
    Event,
};

impl Runner {
    pub(in crate::threads::runner) async fn remote_holder(&mut self, frame: &str) {
        let Ok(frame) = serde_json::from_str::<ToRunner>(frame) else {
            return;
        };
        match frame {
            ToRunner::Reserved { request } if self.remote.reserving => {
                self.remote_reserved(request).await;
            }
            ToRunner::Ask { ask } if self.remote.active.as_ref() == Some(ask.request()) => {
                let request = *ask.request();
                let home = self.remote_home();
                let Some(sender) = self.remote.events.clone() else {
                    return;
                };
                tokio::spawn(async move {
                    let result = match super::polling::enrolled(home).await {
                        Some(delivery) => delivery.relay(ask.generation(), ask).await,
                        None => Answer::Failed(crate::hub::answer::Cause::Unreadable),
                    };
                    let _ = sender.send(Event::Relay { request, result }).await;
                });
            }
            ToRunner::Created { request, process } => {
                self.remote_started(request, process).await;
            }
            ToRunner::Pending { request } if self.remote.active == Some(request) => {
                self.remote.active = None;
            }
            ToRunner::Refused => self.remote_refused(),
            _ => {}
        }
    }

    async fn remote_reserved(&mut self, request: DispatchRequest) {
        self.remote.reserving = false;
        self.remote.active = Some(request);
        let home = self.remote_home();
        let store = Store::at(&home);
        let Ok(saved) = store.reservation(&request) else {
            return;
        };
        if saved.progress() != Progress::Reserved {
            self.remote.active = None;
            if saved.progress() != Progress::Closed(NotCreated::Refused) {
                self.remote_send(ToHolder::Recover { request }).await;
            }
            return;
        }
        let Some(policy) = self.remote_policy(&home) else {
            self.remote_send(ToHolder::Close {
                request,
                cause: NotCreated::Refused,
            })
            .await;
            return;
        };
        let candidates = self.remote_targets(&policy);
        let access = policy.check_launch(saved.project(), saved.parameters(), &candidates);
        let start = access
            .ok()
            .and_then(|access| self.remote_order(&saved, access, candidates));
        match start {
            Some(start) if self.written() => {
                self.remote_send(ToHolder::Start(Box::new(start))).await;
            }
            _ => {
                self.remote_send(ToHolder::Close {
                    request,
                    cause: NotCreated::Refused,
                })
                .await;
            }
        }
    }

    fn remote_order(
        &mut self,
        saved: &crate::remote_code::launch::Reservation,
        mode: Mode,
        candidates: Vec<crate::remote_code::policy::Target>,
    ) -> Option<Start> {
        self.may_start(0).ok()?;
        let request = *saved.request();
        if self.threads.contains_key(&request.runner) {
            return None;
        }
        let parameters = saved.parameters();
        let path = login::find(
            parameters.program,
            &self.env,
            login::executable,
            login::trusted,
        )?;
        let new = NewThread {
            program: parameters.program,
            provider_address: None,
            folder: parameters.folder.clone(),
            mode,
            model: None,
            effort: None,
            speed: None,
            prompt: String::new(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
            worktree: None,
        };
        let (row, action) = self.prepared(&new, request.runner, &path).ok()?;
        let Action::Start { program, env, .. } = action else {
            return None;
        };
        let codex = codex_of(&row, None, self.remote_home().dir());
        self.rights_copy(request.runner, path);
        self.threads
            .insert(request.runner, Thread::new(row, None, codex));
        self.rights_now();
        if self.shared_root(request.runner).is_some() {
            self.threads.remove(&request.runner);
            self.rights_now();
            return None;
        }
        Some(Start {
            request,
            program,
            env,
            mode,
            candidates,
        })
    }

    async fn remote_started(
        &mut self,
        request: DispatchRequest,
        process: crate::remote_code::launch::ProcessIdentity,
    ) {
        let runner = request.runner;
        let store = Store::at(&self.remote_home());
        let Ok(saved) = store.reservation(&request) else {
            return;
        };
        if !saved.matches_process(&process) {
            return;
        }
        if self.remote.active == Some(request) {
            self.remote.active = None;
        }
        if self
            .remote
            .live
            .insert(runner, (request, process))
            .is_some()
        {
            return;
        }
        let Some(thread) = self.threads.get_mut(&runner) else {
            return;
        };
        if thread.row.state == RunState::Stopped {
            return;
        }
        let steps = thread.begin_remote();
        let row = thread.row.clone();
        self.publish(
            runner,
            Change::State {
                state: row.state,
                row: Box::new(row),
            },
        );
        let _ = self.apply(runner, steps).await;
        self.save();
        self.remote_publish(request);
    }

    fn remote_refused(&mut self) {
        self.remote.reserving = false;
        if let Some(request) = self.remote.active.take() {
            self.threads.remove(&request.runner);
            self.rights_now();
            self.save();
        }
    }

    pub(super) async fn remote_send(&mut self, frame: ToHolder) {
        if let Some(action) = frame.action()
            && self.command(action).await.is_none()
        {
            self.remote.reserving = false;
            self.remote.active = None;
        }
    }

    pub(super) fn remote_home(&self) -> Home {
        Home::at(self.dir.parent().unwrap_or(&self.dir).to_owned())
    }

    pub(super) fn remote_policy(&self, home: &Home) -> Option<Policy> {
        let user = crate::readers::agent_sessions::user_home(|name| {
            self.env
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        })?;
        Some(Policy::read(home, user))
    }

    pub(super) fn remote_targets(
        &self,
        policy: &Policy,
    ) -> Vec<crate::remote_code::policy::Target> {
        policy
            .configured_targets()
            .into_iter()
            .filter(|target| {
                login::find(target.program, &self.env, login::executable, login::trusted).is_some()
            })
            .collect()
    }

    pub(super) fn remote_publish(&mut self, request: DispatchRequest) {
        let Some(row) = self
            .threads
            .get(&request.runner)
            .filter(|thread| thread.remote_ready())
            .map(|thread| thread.row.clone())
        else {
            return;
        };
        let now = tokio::time::Instant::now();
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
            .insert(request.runner, now + std::time::Duration::from_secs(10));
        let home = self.remote_home();
        tokio::spawn(async move {
            if let Some(delivery) = super::polling::enrolled(home.clone()).await {
                publish(&home, &delivery, request, row).await;
            }
        });
    }
}

async fn publish(
    home: &Home,
    delivery: &Delivery,
    request: DispatchRequest,
    row: agent_threads::control::ThreadRow,
) {
    let allowed = || crate::runtime::thread_sync::remote_publishable(home, &row);
    if allowed() && delivery.publish_summary(home, &row, &request).await && allowed() {
        let _ = delivery.report_started(&request).await;
    }
    if !allowed() && Store::at(home).withdraw(&request).is_err() {
        return;
    }
    let _ = delivery.evidence(&request).await;
}
