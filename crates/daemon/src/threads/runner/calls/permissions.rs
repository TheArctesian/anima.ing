use agent_threads::control::Action;

/// Whether only the person may make this call: an answer to a request, a raise of the limit,
/// a start, a change of how a thread is listed (TH46), the queue of a thread (TH109) and a change
/// of its model, effort or mode (TH110), and the files of a prompt (TH111). A process of an agent may not, so that an agent never
/// approves itself, gives itself more rights or hides a thread from the person.
pub(super) const fn for_the_person(action: &Action) -> bool {
    if let Action::Git { action, .. } = action {
        return action.for_the_person();
    }
    matches!(
        action,
        Action::Answer { .. }
            | Action::RaiseLimit
            | Action::Start { .. }
            | Action::StartManaged { .. }
            | Action::ChangeListing { .. }
            | Action::ChangeProject { .. }
            | Action::CreateProject { .. }
            | Action::CloneProject { .. }
            | Action::Files { .. }
            | Action::Folders { .. }
            | Action::Queue { .. }
            | Action::Requeue { .. }
            | Action::Unqueue { .. }
            | Action::SendQueued { .. }
            | Action::Set { .. }
            | Action::Commands { .. }
            | Action::Compact { .. }
            | Action::Attach { .. }
            | Action::AttachMore { .. }
            | Action::Unattach { .. }
    )
}
