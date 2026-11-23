//! What a person runs or adds to make a change by hand when this program refused it because a
//! different user can change the agent's program or file (R-442).

use std::path::Path;

use super::super::agents::NAME;
use super::Action;

/// The sentence that tells a person to run the agent's own program themselves.
pub(super) fn command(action: Action, program: &Path, arguments: &[String]) -> String {
    let words: Vec<String> = std::iter::once(program.to_string_lossy().into_owned())
        .chain(arguments.iter().cloned())
        .map(|word| quoted(&word))
        .collect();
    format!("{}, run: {}", yourself(action), words.join(" "))
}

/// The sentence that tells a person to change the agent's file themselves: the member `value` to
/// add to the object at `keys`, or the member to take out.
pub(super) fn member(action: Action, file: &Path, keys: &[&str], value: &str) -> String {
    let object = keys
        .iter()
        .map(|key| format!("\"{key}\""))
        .collect::<Vec<_>>()
        .join(" > ");
    let file = file.display();
    match action {
        Action::Install => format!(
            "{}, add this member to the object {object} of {file}: \"{NAME}\": {value}",
            yourself(action)
        ),
        Action::Uninstall => format!(
            "{}, delete the member \"{NAME}\" of the object {object} of {file}",
            yourself(action)
        ),
    }
}

const fn yourself(action: Action) -> &'static str {
    match action {
        Action::Install => "To set it up yourself",
        Action::Uninstall => "To take it out yourself",
    }
}

/// `word` as a POSIX shell reads it back: as it is when it holds only plain characters, else in
/// single quotes.
pub(super) fn quoted(word: &str) -> String {
    let plain =
        |character: char| character.is_ascii_alphanumeric() || "/._-=:@%+,".contains(character);
    if !word.is_empty() && word.chars().all(plain) {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}
