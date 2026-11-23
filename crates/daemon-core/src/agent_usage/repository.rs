//! The name of a session's repository (AIS-002, R-134). A remote that is a local path has no
//! name, so no folder path is ever sent (AIS-018). Pure: the daemon reads the files.

use std::path::{Component, Path};

use super::is_repository;

const SCHEMES: [&str; 6] = ["https", "http", "ssh", "git", "git+ssh", "ssh+git"];

#[must_use]
pub fn origin_url(config: &str) -> Option<&str> {
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[') {
            in_origin = is_origin_header(header);
            continue;
        }
        if !in_origin {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("url") {
            return Some(value_text(value)).filter(|url| !url.is_empty());
        }
    }
    None
}

fn is_origin_header(header: &str) -> bool {
    let Some((inside, _)) = header.split_once(']') else {
        return false;
    };
    let inside = inside.trim();
    if inside.eq_ignore_ascii_case("remote.origin") {
        return true;
    }
    let Some((section, subsection)) = inside.split_once(char::is_whitespace) else {
        return false;
    };
    section.eq_ignore_ascii_case("remote") && subsection.trim() == "\"origin\""
}

fn value_text(value: &str) -> &str {
    let value = value.trim();
    if let Some(quoted) = value.strip_prefix('"') {
        return quoted.split('"').next().unwrap_or_default();
    }
    value.split(['#', ';']).next().unwrap_or_default().trim()
}

/// `None` for a local path, a `file://` URL, an scp path that starts with `/`, or a name that
/// breaks [`is_repository`], so a folder path on the host is never sent (AIS-018).
#[must_use]
pub fn remote_name(url: &str) -> Option<String> {
    let url = url.trim();
    let (authority, path) = if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if !SCHEMES.contains(&scheme.as_str()) {
            return None;
        }
        rest.split_once('/')?
    } else {
        // The scp form `[user@]host:path`; a local path has a `/` before any `:`, and a path
        // that starts with `/` after the `:` is a folder of the host.
        let (authority, path) = url.split_once(':')?;
        if authority.is_empty() || authority.contains('/') || path.starts_with('/') {
            return None;
        }
        (authority, path)
    };
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host_port
        .split_once(':')
        .map_or(host_port, |(host, _)| host)
        .to_ascii_lowercase();
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let name = format!("{host}/{path}");
    is_repository(&name).then_some(name)
}

#[must_use]
pub fn gitdir_link(text: &str) -> Option<&str> {
    let folder = text.trim().strip_prefix("gitdir:")?.trim();
    (!folder.is_empty()).then_some(folder)
}

/// The folder above `.claude/worktrees/<name>`: the worktree may be gone when the daemon reads
/// the session, the main folder seldom is.
#[must_use]
pub fn claude_worktree_main(cwd: &Path) -> Option<&Path> {
    let parts: Vec<Component> = cwd.components().collect();
    let at = parts.windows(3).position(|three| {
        three[0].as_os_str() == ".claude"
            && three[1].as_os_str() == "worktrees"
            && matches!(three[2], Component::Normal(_))
    })?;
    let mut main = cwd;
    for _ in at..parts.len() {
        main = main.parent()?;
    }
    Some(main)
}
