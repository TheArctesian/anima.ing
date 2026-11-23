use std::{borrow::Cow, fmt};

/// A Guacamole instruction: an opcode and its arguments, in wire order.
/// `Debug` prints the opcode and the argument count only: no argument value may reach a log (LOG-007).
#[derive(Clone, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: String,
    pub args: Vec<String>,
}

/// Longest prefix of a peer-supplied opcode or name that `Debug` and errors print.
pub(crate) const NAME_CHARS: usize = 32;

impl Instruction {
    #[must_use]
    pub fn new<A>(opcode: impl Into<String>, args: A) -> Self
    where
        A: IntoIterator,
        A::Item: Into<String>,
    {
        Self {
            opcode: opcode.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    /// Each character outside the BMP becomes U+FFFD, for the client-bound encode only:
    /// guacamole-common-js before 1.5.2 counts lengths in UTF-16 units (K4 Section 4, R-046).
    #[must_use]
    pub fn with_bmp_text(&self) -> Cow<'_, Self> {
        if is_bmp(&self.opcode) && self.args.iter().all(|arg| is_bmp(arg)) {
            return Cow::Borrowed(self);
        }
        Cow::Owned(Self {
            opcode: bmp_text(&self.opcode),
            args: self.args.iter().map(|arg| bmp_text(arg)).collect(),
        })
    }
}

/// In UTF-8 only characters outside the BMP start with a byte of 0xF0 or more.
fn is_bmp(text: &str) -> bool {
    text.bytes().all(|byte| byte < 0xF0)
}

fn bmp_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.len_utf16() == 1 {
                c
            } else {
                char::REPLACEMENT_CHARACTER
            }
        })
        .collect()
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instruction")
            .field("opcode", &truncated(&self.opcode, NAME_CHARS))
            .field("arg_count", &self.args.len())
            .finish()
    }
}

/// Peer opcodes, names and messages are bounded before they reach `Debug` or an error message.
pub(crate) fn truncated(text: &str, max_chars: usize) -> String {
    let mut prefix: String = text.chars().take(max_chars).collect();
    if text.chars().nth(max_chars).is_some() {
        prefix.push('…');
    }
    prefix
}

/// Encodes `i` as `LENGTH.VALUE` elements separated by `,` and ended by `;`.
///
/// `LENGTH` counts Unicode code points, as guacd does. The encoder does not
/// apply the [`Decoder`](crate::Decoder) limits; the caller builds instructions
/// that fit them.
#[must_use]
pub fn encode(i: &Instruction) -> String {
    let mut out = String::new();
    push_element(&mut out, &i.opcode);
    for arg in &i.args {
        out.push(',');
        push_element(&mut out, arg);
    }
    out.push(';');
    out
}

fn push_element(out: &mut String, value: &str) {
    out.push_str(&value.chars().count().to_string());
    out.push('.');
    out.push_str(value);
}
