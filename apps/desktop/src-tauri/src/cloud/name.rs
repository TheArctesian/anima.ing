const MOST: usize = 40;

/// The cloud takes 1 to 40 characters of `a-z`, `0-9` and `-`, starting with a letter.
pub(super) fn device_name(computer: &str) -> String {
    let mut name = String::new();
    for character in computer.trim().to_lowercase().chars() {
        let kept = if character.is_ascii_lowercase() || character.is_ascii_digit() {
            character
        } else {
            '-'
        };
        if !(kept == '-' && (name.is_empty() || name.ends_with('-'))) {
            name.push(kept);
        }
    }
    let name = name.trim_end_matches('-');
    let name = if name.starts_with(|c: char| c.is_ascii_lowercase()) {
        name.to_owned()
    } else if name.is_empty() {
        "computer".to_owned()
    } else {
        format!("computer-{name}")
    };
    name.chars()
        .take(MOST)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}
