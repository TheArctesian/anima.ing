//! Message templates (NOT-007, D-21): placeholder substitution and nothing else.
//!
//! A template is parsed when it is saved, so a mistake is a `VALIDATION` error, not a failed send.
//! `{{` and `}}` are literal braces, a missing value renders empty, and a value is never parsed again.

#[cfg(test)]
mod tests;
mod values;

pub(super) use values::Values;

use crate::{api_errors::FieldError, text::clean};

/// The longest template title and body, in characters (K2).
pub(super) const TITLE_MAX: usize = 500;
pub(super) const BODY_MAX: usize = 4000;

/// The longest unknown name an error message repeats, cleaned of control characters (LOG-007).
const NAME_SHOWN: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Placeholder {
    Device,
    Check,
    Status,
    Previous,
    Message,
    At,
    Days,
}

impl Placeholder {
    const ALL: [Self; 7] = [
        Self::Device,
        Self::Check,
        Self::Status,
        Self::Previous,
        Self::Message,
        Self::At,
        Self::Days,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Check => "check",
            Self::Status => "status",
            Self::Previous => "previous",
            Self::Message => "message",
            Self::At => "at",
            Self::Days => "days",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }
}

pub(super) fn placeholder_texts() -> Vec<String> {
    Placeholder::ALL
        .into_iter()
        .map(|p| format!("{{{}}}", p.name()))
        .collect()
}

/// The messages quote at most a short name, never the template.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum Invalid {
    #[error("{{{0}}} is not a placeholder")]
    Unknown(String),
    #[error("a {{ has no closing }}")]
    Unclosed,
    #[error("a }} has no opening {{")]
    Stray,
}

enum Piece {
    Text(String),
    Field(Placeholder),
}

pub(super) struct Template(Vec<Piece>);

impl Template {
    pub(super) fn parse(text: &str) -> Result<Self, Invalid> {
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.next_if_eq(&'{').is_some() => literal.push('{'),
                '}' if chars.next_if_eq(&'}').is_some() => literal.push('}'),
                '}' => return Err(Invalid::Stray),
                '{' => {
                    let mut name = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some('{') | None => return Err(Invalid::Unclosed),
                            Some(c) => name.push(c),
                        }
                    }
                    let field = Placeholder::from_name(&name)
                        .ok_or_else(|| Invalid::Unknown(clean(&name, NAME_SHOWN)))?;
                    if !literal.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(&mut literal)));
                    }
                    pieces.push(Piece::Field(field));
                }
                _ => literal.push(c),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Text(literal));
        }
        Ok(Self(pieces))
    }

    pub(super) fn render(&self, values: &Values) -> String {
        self.0
            .iter()
            .map(|piece| match piece {
                Piece::Text(text) => text.clone(),
                Piece::Field(field) => values.get(*field),
            })
            .collect()
    }
}

/// Not encoded: each channel encodes the title and body for itself (`encode`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Rendered {
    pub(super) title: String,
    pub(super) body: String,
}

pub(super) fn render(title: &str, body: &str, values: &Values) -> Result<Rendered, Invalid> {
    let cut = |text: String, max: usize| text.chars().take(max).collect::<String>();
    Ok(Rendered {
        title: cut(Template::parse(title)?.render(values), TITLE_MAX),
        body: cut(Template::parse(body)?.render(values), BODY_MAX),
    })
}

/// A reminder's own title and body: no rule text is parsed as a template (REM-009, REM-010).
pub(super) fn given(values: &Values) -> Rendered {
    Rendered {
        title: values.title.clone().unwrap_or_default(),
        body: values.body.clone().unwrap_or_default(),
    }
}

/// Field names match K1's `NotificationTemplateInput`.
pub(super) fn field_errors(title: &str, body: &str) -> Vec<FieldError> {
    let mut errors = Vec::new();
    for (field, text, max) in [
        ("template.title", title, TITLE_MAX),
        ("template.body", body, BODY_MAX),
    ] {
        if text.chars().count() > max {
            errors.push(FieldError::new(
                field,
                format!("Use at most {max} characters."),
            ));
        } else if let Err(invalid) = Template::parse(text) {
            errors.push(FieldError::new(field, explain(&invalid)));
        }
    }
    errors
}

fn explain(invalid: &Invalid) -> String {
    let names = placeholder_texts().join(", ");
    format!("{invalid}. Use {names}. Write {{{{ for a {{ and }}}} for a }}.")
}
