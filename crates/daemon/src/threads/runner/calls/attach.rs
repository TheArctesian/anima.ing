//! The calls `attach`, `attach_more` and `unattach` (TH111): the person sends a file for a
//! prompt in parts, or drops it. Only the person makes them; the log names no file.

use agent_threads::control::{Action, Change, Outcome, Refusal};
use agent_threads::drive::{DriveEvent, Prompt};
use uuid::Uuid;

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
                draft,
            } => {
                match self.threads.get(&thread) {
                    // A key of a draft is never the ID of a thread.
                    Some(_) if draft => return Err(Refusal::BadValue),
                    None if draft => {}
                    Some(thread) => thread.can_take_a_prompt()?,
                    None => return Err(Refusal::UnknownThread),
                }
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

impl Runner {
    /// The first prompt of a new thread with the files of its draft (TH111). Files that cannot
    /// go stay with the thread for a later prompt, and the thread says so; the text goes.
    pub(in crate::threads::runner) fn first_prompt(
        &mut self,
        id: Uuid,
        text: &str,
        files: &[Uuid],
    ) -> Prompt {
        let mut prompt = Prompt::plain(text);
        let Some(program) = self
            .threads
            .get(&id)
            .map(super::super::thread::Thread::program)
        else {
            return prompt;
        };
        if files.is_empty() {
            return prompt;
        }
        match self.attached.take(id, files, program) {
            Ok(attachments) => prompt.attachments = attachments,
            Err(why) => {
                let said = match why {
                    Refusal::CannotAttach { said } | Refusal::TooLarge { said } => said,
                    _ => "The files could not go.".to_owned(),
                };
                let text = format!(
                    "The first prompt went with no file. {said} The files stay attached: send them with the next prompt."
                );
                let event = DriveEvent::Notice { text };
                self.publish(id, Change::Said { event });
            }
        }
        prompt
    }

    /// The files `taken` for the first prompt went with its line, or wait again when the line
    /// did not go.
    pub(in crate::threads::runner) fn first_files_went(
        &mut self,
        id: Uuid,
        taken: &[Uuid],
        went: bool,
    ) {
        if taken.is_empty() {
            return;
        }
        let files = taken;
        if went {
            self.attached.settle(files);
            self.note(Kind::Attach, Some(id), "files went with the first prompt");
        } else {
            self.attached.restore(id, files);
        }
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
                draft: false,
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
