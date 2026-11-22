//! The calls `attach`, `attach_more` and `unattach` (TH111): the person sends a file for a
//! prompt in parts, or drops it. Only the person makes them; the log names no file.

use agent_threads::control::{Action, Outcome, Refusal};

use super::super::Runner;
use super::super::attachments::NewFile;
use super::super::log::Kind;

impl Runner {
    pub(super) fn attach_call(&mut self, action: Action) -> Outcome {
        match self.attach_or_refuse(action) {
            Ok(outcome) => outcome,
            Err(why) => Outcome::Refused { why },
        }
    }

    fn attach_or_refuse(&mut self, action: Action) -> Result<Outcome, Refusal> {
        let (thread, id, bytes) = match action {
            Action::Attach {
                thread,
                name,
                mime,
                size,
                bytes,
            } => {
                self.threads
                    .get(&thread)
                    .ok_or(Refusal::UnknownThread)?
                    .can_take_a_prompt()?;
                let file = NewFile {
                    name: &name,
                    mime: &mime,
                    size,
                };
                let id = self.attached.begin(thread, file)?;
                (thread, id, bytes)
            }
            Action::AttachMore {
                thread,
                attachment,
                bytes,
            } => (thread, attachment, bytes),
            Action::Unattach { thread, attachment } => {
                self.attached.drop_file(thread, attachment)?;
                return Ok(Outcome::Ok);
            }
            _ => return Err(Refusal::BadValue),
        };
        let received = self.attached.add(thread, id, &bytes)?;
        if self.attached.complete(id) {
            self.note(Kind::Attach, Some(thread), "a file came");
        }
        Ok(Outcome::Attached { id, received })
    }
}

#[cfg(test)]
mod tests {
    use agent_threads::control::Action;
    use uuid::Uuid;

    #[test]
    fn th111_only_the_person_attaches_a_file() {
        let thread = Uuid::nil();
        for action in [
            Action::Attach {
                thread,
                name: "a".to_owned(),
                mime: "text/plain".to_owned(),
                size: 1,
                bytes: "YQ==".to_owned(),
            },
            Action::AttachMore {
                thread,
                attachment: thread,
                bytes: String::new(),
            },
            Action::Unattach {
                thread,
                attachment: thread,
            },
        ] {
            assert!(super::super::for_the_person(&action), "{action:?}");
        }
    }
}
