//! A command made only from static text and values the hub renders itself (CHK-012, MET-002,
//! D-3, HOST-001).

use crate::fleet::devices::MacAddress;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// A full Docker container identifier.
const MAX_HEX_DIGITS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Rendered {
    Static(&'static str),
    Mac(MacAddress),
    Hex(HexDigits),
    Number(u32),
}

/// Kept as values, so the word they render to is only `0-9a-f` (HOST-001).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HexDigits(Vec<u8>);

impl HexDigits {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() || text.len() > MAX_HEX_DIGITS {
            return None;
        }
        text.bytes()
            .map(|byte| {
                let value = HEX.iter().position(|digit| *digit == byte)?;
                u8::try_from(value).ok()
            })
            .collect::<Option<Vec<u8>>>()
            .map(Self)
    }
}

/// No constructor or method takes text that is not `'static`, so operator text cannot enter
/// the line (CHK-012).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedCommand {
    program: &'static str,
    args: Vec<Rendered>,
}

impl FixedCommand {
    #[must_use]
    pub const fn new(program: &'static str) -> Self {
        Self {
            program,
            args: Vec::new(),
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: &'static str) -> Self {
        self.args.push(Rendered::Static(arg));
        self
    }

    #[must_use]
    pub fn mac(mut self, mac: MacAddress) -> Self {
        self.args.push(Rendered::Mac(mac));
        self
    }

    #[must_use]
    pub fn hex(mut self, digits: &HexDigits) -> Self {
        self.args.push(Rendered::Hex(digits.clone()));
        self
    }

    #[must_use]
    pub fn number(mut self, number: u32) -> Self {
        self.args.push(Rendered::Number(number));
        self
    }

    pub(super) fn line(&self) -> String {
        let mut line = self.program.to_owned();
        for arg in &self.args {
            line.push(' ');
            match arg {
                Rendered::Static(text) => line.push_str(text),
                Rendered::Mac(mac) => push_mac(&mut line, *mac),
                Rendered::Hex(digits) => push_hex(&mut line, digits),
                Rendered::Number(number) => line.push_str(&number.to_string()),
            }
        }
        line
    }
}

/// From the bytes, so the line does not depend on how `MacAddress` displays itself.
fn push_mac(line: &mut String, mac: MacAddress) {
    for (index, octet) in mac.octets().iter().enumerate() {
        if index > 0 {
            line.push(':');
        }
        line.push(char::from(HEX[usize::from(octet >> 4)]));
        line.push(char::from(HEX[usize::from(octet & 0x0f)]));
    }
}

fn push_hex(line: &mut String, digits: &HexDigits) {
    for value in &digits.0 {
        line.push(char::from(HEX[usize::from(value & 0x0f)]));
    }
}
