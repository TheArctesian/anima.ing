//! The slug of a new project, its address in the client (`/projects/<slug>`).
//! A slug never changes after the project is made, so a rename upstream keeps the address.

const NAME_CHARS: usize = 64;
const OWNER_CHARS: usize = 40;
const NAMELESS: &str = "project";

fn plain(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if out.len() == max {
                break;
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            if out.len() + 1 >= max {
                break;
            }
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[must_use]
pub fn for_repository(full_name: &str, taken: &[String]) -> String {
    let (owner, name) = full_name.split_once('/').unwrap_or(("", full_name));
    let mut base = plain(name, NAME_CHARS);
    if base.is_empty() {
        NAMELESS.clone_into(&mut base);
    }
    let free = |slug: &str| !taken.iter().any(|t| t == slug);
    if free(&base) {
        return base;
    }
    let owner = plain(owner, OWNER_CHARS);
    let with_owner = if owner.is_empty() {
        base
    } else {
        format!("{base}-{owner}")
    };
    if free(&with_owner) {
        return with_owner;
    }
    // Among one more number than there are taken slugs, one is free.
    let last = u32::try_from(taken.len())
        .unwrap_or(u32::MAX)
        .saturating_add(2);
    (2..=last)
        .map(|n| format!("{with_owner}-{n}"))
        .find(|slug| free(slug))
        .unwrap_or(with_owner)
}

/// Every slug [`for_repository`] gives starts with this, so the store reads only those.
#[must_use]
pub fn stem(full_name: &str) -> String {
    let name = full_name
        .split_once('/')
        .map_or(full_name, |(_, name)| name);
    let base = plain(name, NAME_CHARS);
    if base.is_empty() {
        NAMELESS.to_owned()
    } else {
        base
    }
}
