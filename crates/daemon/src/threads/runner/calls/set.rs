//! The call `set` (TH110): the person changes the model, the effort or the mode of a thread from
//! its next turn. Only the person makes it: a mode can give the agent more rights.

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::Model;

use super::super::Runner;
use super::super::accounts::Asked;
use super::super::log::Kind;
use super::super::thread::Changes;

impl Runner {
    pub(super) async fn set(&mut self, action: Action) -> Outcome {
        let refused = |why| Outcome::Refused { why };
        let Action::Set {
            thread: id,
            model,
            effort,
            mode,
            speed,
        } = action
        else {
            return refused(Refusal::BadValue);
        };
        // A model name becomes an argument of a later start: it keeps its alphabet (THR-040).
        let Ok(model) = model.as_deref().map(Model::new).transpose() else {
            return refused(Refusal::BadValue);
        };
        if model.is_none() && effort.is_none() && mode.is_none() && speed.is_none() {
            return refused(Refusal::BadValue);
        }
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::UnknownThread);
        };
        let named = model
            .as_ref()
            .map(Model::as_str)
            .or(thread.row.model.as_deref());
        let asked = Asked {
            model: named,
            effort,
            speed: speed.as_deref(),
        };
        if !self.said.models.offers(thread.row.program, &asked) {
            return refused(Refusal::BadValue);
        }
        match thread.set(Changes {
            model,
            effort,
            mode,
            speed,
        }) {
            Ok(steps) => {
                self.note(
                    Kind::Change,
                    Some(id),
                    "model, effort, speed or mode changed",
                );
                if self.apply(id, steps).await.is_err() {
                    return refused(Refusal::Holder);
                }
                self.save();
                Outcome::Ok
            }
            Err(why) => refused(why),
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_threads::control::Action;
    use agent_threads::drive::Mode;
    use uuid::Uuid;

    #[test]
    fn th110_only_the_person_changes_the_model_the_effort_or_the_mode() {
        let set = Action::Set {
            thread: Uuid::nil(),
            model: None,
            effort: None,
            mode: Some(Mode::FullAccess),
            speed: None,
        };
        assert!(super::super::for_the_person(&set));
    }
}
