//! The commands of a service (SF-251): a shell in one can run any text, so it needs a person who read it.

use compose_rules::Node;

const SHELLS: [&str; 7] = ["sh", "bash", "ash", "dash", "zsh", "busybox", "env"];
/// Interpreters that run code given in a word, with the short and long options that take it (SF-252).
const INLINE: [(&str, &str, &[&str]); 9] = [
    ("perl", "eE", &[]),
    ("node", "ep", &["--eval", "--print"]),
    ("nodejs", "ep", &["--eval", "--print"]),
    ("bun", "ep", &["--eval", "--print"]),
    ("python", "c", &[]),
    ("pypy", "c", &[]),
    ("ruby", "e", &[]),
    ("php", "r", &[]),
    ("lua", "e", &[]),
];
/// The interpreters whose subcommand `eval` runs code given in a word.
const EVAL: [&str; 1] = ["deno"];
/// An awk program that calls `system`, reads with `getline` or uses a pipe `|` runs a command.
const AWK: [&str; 4] = ["awk", "gawk", "mawk", "nawk"];
const AWK_COMMAND: [&str; 3] = ["system", "getline", "|"];
/// The variables of the init of `LinuxServer` that get or install code at the start.
const MODS: [&str; 3] = ["DOCKER_MODS", "INSTALL_PACKAGES", "INSTALL_PIP_PACKAGES"];

fn split(node: Option<&Node>) -> Vec<&str> {
    match node {
        Some(Node::Text(text)) => text.split_whitespace().collect(),
        Some(Node::List(items)) => items
            .iter()
            .filter_map(Node::text)
            .flat_map(str::split_whitespace)
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn entrypoint(service: &Node) -> Vec<&str> {
    split(service.get("entrypoint"))
}

/// The words of the health check's test, and whether a shell runs them (one text, or `CMD-SHELL`).
fn health(service: &Node) -> (Vec<&str>, bool) {
    let test = service
        .get("healthcheck")
        .and_then(|check| check.get("test"));
    match test {
        Some(Node::Text(_)) => (split(test), true),
        Some(Node::List(items)) => {
            let words = items
                .iter()
                .skip(1)
                .filter_map(Node::text)
                .flat_map(str::split_whitespace)
                .collect();
            let shell = items.first().and_then(Node::text) == Some("CMD-SHELL");
            (words, shell)
        }
        _ => (Vec::new(), false),
    }
}

pub(crate) fn words(service: &Node) -> Vec<&str> {
    let mut found = entrypoint(service);
    found.extend(split(service.get("command")));
    found.extend(health(service).0);
    found
}

fn is_shell(word: &str) -> bool {
    let program = word.rsplit('/').next().unwrap_or(word);
    SHELLS.contains(&program) || word == "-c"
}

/// The inline-code options of the interpreter of `word`, if any: `python3.12` is `python`.
fn inline_options(word: &str) -> Option<(&'static str, &'static [&'static str])> {
    let bare = program(word)
        .trim_end_matches(|character: char| character.is_ascii_digit() || character == '.');
    INLINE
        .iter()
        .find(|(name, _, _)| *name == bare)
        .map(|(_, short, long)| (*short, *long))
}

/// Whether `option` takes inline code: a short cluster such as `-le` with a letter of `short`, or a `long`.
fn is_inline_option(option: &str, short: &str, long: &[&str]) -> bool {
    match option.strip_prefix("--") {
        Some(_) => long
            .iter()
            .any(|name| option == *name || option.starts_with(&format!("{name}="))),
        None => option
            .strip_prefix('-')
            .is_some_and(|letters| letters.chars().any(|letter| short.contains(letter))),
    }
}

fn program(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// Whether an interpreter in `words` runs code given in a word, as `perl -e` or `deno eval` do.
fn runs_inline_code(words: &[&str]) -> bool {
    words.iter().enumerate().any(|(at, word)| {
        let after = &words[at + 1..];
        let inline = inline_options(word).is_some_and(|(short, long)| {
            after
                .iter()
                .any(|option| is_inline_option(option, short, long))
        });
        let eval = EVAL.contains(&program(word)) && after.first() == Some(&"eval");
        let awk = AWK.contains(&program(word))
            && (after.iter()).any(|text| AWK_COMMAND.iter().any(|call| text.contains(call)));
        inline || eval || awk
    })
}

pub(crate) fn runs_shell(service: &Node) -> bool {
    let (test, shell) = health(service);
    let found: Vec<&str> = split(service.get("command"))
        .into_iter()
        .chain(test)
        .collect();
    shell || found.iter().copied().any(is_shell) || runs_inline_code(&found)
}

pub(crate) fn sets_mods(service: &Node) -> bool {
    words(service)
        .into_iter()
        .any(|word| MODS.iter().any(|name| word.contains(name)))
}

/// Whether a word gives a value to a secret name; a name that ends with `FILE` names a path, not a secret.
pub(crate) fn secret_literal(service: &Node, is_secret_name: fn(&str) -> bool) -> bool {
    let is_secret =
        |name: &str| is_secret_name(name) && !name.to_ascii_uppercase().ends_with("FILE");
    let found = words(service);
    found.iter().enumerate().any(|(at, word)| {
        let bare = word.trim_start_matches('-');
        match bare.split_once('=') {
            Some((name, value)) => is_secret(name) && !value.is_empty(),
            None => {
                word.starts_with('-')
                    && is_secret(bare)
                    && found.get(at + 1).is_some_and(|next| !next.starts_with('-'))
            }
        }
    })
}
