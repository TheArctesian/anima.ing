//! Recognize only the provider's device challenge; never forward arbitrary program output.
use zeroize::Zeroizing;

pub(super) const VERIFICATION: &str = "https://auth.openai.com/codex/device";

pub(super) fn codex(bytes: &[u8]) -> Option<Zeroizing<String>> {
    let raw = std::str::from_utf8(bytes).ok()?;
    let mut plain = Zeroizing::new(String::new());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.next()? != '[' {
                return None;
            }
            let mut complete = false;
            for ch in chars.by_ref().take(32) {
                if ('@'..='~').contains(&ch) {
                    complete = true;
                    break;
                }
            }
            if !complete {
                return None;
            }
        } else if ch == '\n' || ch == '\r' || !ch.is_control() {
            plain.push(ch);
        }
    }
    if !plain.lines().any(|line| line.trim() == VERIFICATION) {
        return None;
    }
    let (_, after) = plain.split_once("2. Enter this one-time code")?;
    let code = after
        .lines()
        .skip(1)
        .find(|line| !line.trim().is_empty())?
        .trim();
    // Require a complete line: chunk boundaries must not truncate an accepted code.
    if !after.contains(&format!("{code}\n")) && !after.contains(&format!("{code}\r\n")) {
        return None;
    }
    if !(4..=32).contains(&code.len())
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }
    Some(Zeroizing::new(code.to_owned()))
}
