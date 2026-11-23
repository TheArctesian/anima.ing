//! Which stateless Apprise URLs the hub sends (N-4, D-17, D-18).
//!
//! Apprise connects on its own, outside the hub's target rule and deny list, so the hub permits
//! only services that post to one fixed vendor host, and query keys that change how the message
//! looks, never where it goes or what Apprise reads. Self-hosted targets use the hub's own
//! webhook, ntfy and e-mail channels (P3-003).
//!
//! The rules follow how Apprise reads a URL: it splits URLs at a comma or a space, takes the
//! query after the first `?`, splits it at `&` and `;`, and percent-decodes, trims and lowers
//! each key. A comma is refused even though a second URL also needs a second `://`.

const SCHEMES: [&str; 8] = [
    "discord",
    "tgram",
    "slack",
    "pover",
    "pbul",
    "gchat",
    "pagerduty",
    "opsgenie",
];

const KEYS: [&str; 17] = [
    "format", "overflow", "emojis", "to", "tts", "avatar", "footer", "thread", "botname", "silent",
    "preview", "topic", "priority", "sound", "retry", "expire", "blocks",
];

const URL_MAX: usize = 2048;

/// The operator reads a refusal, so it never quotes the URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::notifications) enum Refusal {
    Shape,
    Scheme,
    Key,
}

impl Refusal {
    pub(in crate::fleet::notifications) const fn message(self) -> &'static str {
        match self {
            Self::Shape => {
                "Enter one Apprise URL per entry, with no spaces or commas, at most 2048 characters."
            }
            Self::Scheme => {
                "Use a Discord, Telegram, Slack, Pushover, Pushbullet, Google Chat, PagerDuty or \
                 Opsgenie URL. For other targets add a webhook, ntfy or e-mail channel."
            }
            Self::Key => {
                "Remove options the hub does not send (such as template, attach or verify)."
            }
        }
    }
}

/// Checked at save and again before each send (N-4).
pub(in crate::fleet::notifications) fn permitted(url: &str) -> Result<(), Refusal> {
    let shape_ok = !url.is_empty()
        && url.len() <= URL_MAX
        && !url.chars().any(|c| {
            c.is_control() || c.is_whitespace() || c == ',' || crate::text::is_invisible(c)
        })
        && url.matches("://").count() == 1
        && url.matches('?').count() <= 1;
    if !shape_ok {
        return Err(Refusal::Shape);
    }
    let (scheme, rest) = url.split_once("://").ok_or(Refusal::Shape)?;
    if !SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        return Err(Refusal::Scheme);
    }
    let Some((_, query)) = rest.split_once('?') else {
        return Ok(());
    };
    let keys_ok = query
        .split(['&', ';'])
        .filter(|pair| !pair.is_empty())
        .all(|pair| {
            let key = pair.split_once('=').map_or(pair, |(key, _)| key);
            decoded(key)
                .is_some_and(|key| KEYS.iter().any(|allowed| key.eq_ignore_ascii_case(allowed)))
        });
    if keys_ok { Ok(()) } else { Err(Refusal::Key) }
}

fn decoded(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let hex = bytes.get(index + 1..index + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(byte);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}
