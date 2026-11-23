//! Incremental decoder for the Guacamole wire format (GW-011); a feed may end anywhere.

use std::fmt;

use crate::instruction::Instruction;

/// Largest encoded instruction in bytes, with lengths and separators: guacd's `GUAC_INSTRUCTION_MAX_LENGTH`.
pub const MAX_INSTRUCTION_BYTES: usize = 8192;

/// Largest number of elements in one instruction, opcode included: guacd's `GUAC_INSTRUCTION_MAX_ELEMENTS`.
pub const MAX_ELEMENTS: usize = 128;

/// Largest number of digits in one element length: guacd's `GUAC_INSTRUCTION_MAX_DIGITS`.
pub const MAX_LENGTH_DIGITS: usize = 5;

/// Positions are zero-based byte offsets in the whole stream; no variant carries content (LOG-007).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("expected a length digit or '.' at byte {position}")]
    InvalidLength { position: u64 },
    #[error("element length at byte {position} has more than {limit} digits")]
    TooManyLengthDigits { position: u64, limit: usize },
    #[error("expected ',' or ';' after an element at byte {position}")]
    MissingSeparator { position: u64 },
    /// guacd ends an element at its first NUL, so a NUL means different values to the hub and guacd.
    #[error("element contains a NUL byte at byte {position}")]
    NulByte { position: u64 },
    #[error("element ending at byte {position} is not valid UTF-8")]
    InvalidUtf8 { position: u64 },
    #[error("instruction exceeds {limit} bytes at byte {position}")]
    InstructionTooLong { position: u64, limit: usize },
    #[error("instruction exceeds {limit} elements at byte {position}")]
    TooManyElements { position: u64, limit: usize },
}

#[derive(Debug, Clone, Copy)]
enum Element {
    Length {
        length: usize,
        digits: usize,
    },
    /// Reading the value; `chars` code points have started so far.
    Value {
        length: usize,
        chars: usize,
    },
}

/// After the first error every later `feed` returns it, because the tunnel closes (GW-011).
/// `Debug` never prints buffered bytes.
pub struct Decoder {
    element: Element,
    value: Vec<u8>,
    elements: Vec<String>,
    instruction_bytes: usize,
    position: u64,
    failed: Option<DecodeError>,
}

impl fmt::Debug for Decoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Decoder")
            .field("position", &self.position)
            .field("instruction_bytes", &self.instruction_bytes)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            element: Element::Length {
                length: 0,
                digits: 0,
            },
            value: Vec::new(),
            elements: Vec::new(),
            instruction_bytes: 0,
            position: 0,
            failed: None,
        }
    }

    /// Returns every instruction `bytes` complete; bytes of an incomplete one are kept.
    ///
    /// # Errors
    ///
    /// A [`DecodeError`] when the stream breaks the grammar or a limit; that call's instructions are dropped.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Instruction>, DecodeError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        let mut complete = Vec::new();
        for &byte in bytes {
            match self.step(byte) {
                Ok(Some(instruction)) => complete.push(instruction),
                Ok(None) => {}
                Err(error) => {
                    self.release();
                    self.failed = Some(error.clone());
                    return Err(error);
                }
            }
            self.position = self.position.saturating_add(1);
        }
        Ok(complete)
    }

    fn step(&mut self, byte: u8) -> Result<Option<Instruction>, DecodeError> {
        self.instruction_bytes = self.instruction_bytes.saturating_add(1);
        if self.instruction_bytes > MAX_INSTRUCTION_BYTES {
            return Err(DecodeError::InstructionTooLong {
                position: self.position,
                limit: MAX_INSTRUCTION_BYTES,
            });
        }
        match self.element {
            Element::Length { length, digits } => {
                self.length_byte(byte, length, digits).map(|()| None)
            }
            Element::Value { length, chars } if chars == length && starts_char(byte) => {
                self.end_element(byte)
            }
            Element::Value { .. } if byte == 0 => Err(DecodeError::NulByte {
                position: self.position,
            }),
            Element::Value { length, chars } => {
                let chars = if starts_char(byte) {
                    chars.saturating_add(1)
                } else {
                    chars
                };
                self.element = Element::Value { length, chars };
                self.value.push(byte);
                Ok(None)
            }
        }
    }

    fn length_byte(&mut self, byte: u8, length: usize, digits: usize) -> Result<(), DecodeError> {
        match byte {
            b'0'..=b'9' if digits >= MAX_LENGTH_DIGITS => Err(DecodeError::TooManyLengthDigits {
                position: self.position,
                limit: MAX_LENGTH_DIGITS,
            }),
            b'0'..=b'9' => {
                let length = length
                    .saturating_mul(10)
                    .saturating_add(usize::from(byte - b'0'));
                self.element = Element::Length {
                    length,
                    digits: digits.saturating_add(1),
                };
                Ok(())
            }
            b'.' if digits > 0 => {
                // Each code point takes at least one byte, and a separator follows.
                let least = self
                    .instruction_bytes
                    .saturating_add(length)
                    .saturating_add(1);
                if least > MAX_INSTRUCTION_BYTES {
                    return Err(DecodeError::InstructionTooLong {
                        position: self.position,
                        limit: MAX_INSTRUCTION_BYTES,
                    });
                }
                self.element = Element::Value { length, chars: 0 };
                Ok(())
            }
            _ => Err(DecodeError::InvalidLength {
                position: self.position,
            }),
        }
    }

    fn end_element(&mut self, separator: u8) -> Result<Option<Instruction>, DecodeError> {
        let position = self.position;
        if separator != b',' && separator != b';' {
            return Err(DecodeError::MissingSeparator { position });
        }
        let value = String::from_utf8(std::mem::take(&mut self.value))
            .map_err(|_| DecodeError::InvalidUtf8 { position })?;
        self.elements.push(value);
        self.element = Element::Length {
            length: 0,
            digits: 0,
        };
        if separator == b',' {
            if self.elements.len() >= MAX_ELEMENTS {
                return Err(DecodeError::TooManyElements {
                    position,
                    limit: MAX_ELEMENTS,
                });
            }
            return Ok(None);
        }
        self.instruction_bytes = 0;
        let mut elements = std::mem::take(&mut self.elements).into_iter();
        let opcode = elements.next().unwrap_or_default();
        Ok(Some(Instruction {
            opcode,
            args: elements.collect(),
        }))
    }

    /// Drops the partial instruction so a failed decoder holds no content.
    fn release(&mut self) {
        self.value = Vec::new();
        self.elements = Vec::new();
    }
}

fn starts_char(byte: u8) -> bool {
    byte & 0xC0 != 0x80
}
