//! The YAML reader; it refuses each form that a second reader could read differently.

use yaml_rust2::{
    Event,
    parser::{MarkedEventReceiver, Parser},
    scanner::{Marker, TScalarStyle},
};

use crate::{Refusal, Rule};

const DEEPEST: usize = 32;

/// One value of a file; a plain scalar that YAML reads as no value is `Null`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Null,
    Text(String),
    List(Vec<Node>),
    /// The keys in the order of the file.
    Map(Vec<(String, Node)>),
}

impl Node {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// `Null` has no entries, as YAML writes an empty map.
    #[must_use]
    pub fn entries(&self) -> Option<&[(String, Self)]> {
        match self {
            Self::Map(entries) => Some(entries),
            Self::Null => Some(&[]),
            _ => None,
        }
    }
}

enum Open {
    List(Vec<Node>),
    Map(Vec<(String, Node)>, Option<String>),
}

#[derive(Default)]
struct Reader {
    open: Vec<Open>,
    documents: usize,
    root: Option<Node>,
    refused: bool,
}

impl Reader {
    fn put(&mut self, node: Node) {
        match self.open.last_mut() {
            None => self.root = Some(node),
            Some(Open::List(items)) => items.push(node),
            Some(Open::Map(entries, key)) => match key.take() {
                Some(key) => entries.push((key, node)),
                None => match node {
                    Node::Text(text)
                        if text != "<<" && !entries.iter().any(|(name, _)| *name == text) =>
                    {
                        *key = Some(text);
                    }
                    _ => self.refused = true,
                },
            },
        }
    }

    fn open(&mut self, open: Open) {
        self.open.push(open);
        self.refused |= self.open.len() > DEEPEST;
    }

    fn close(&mut self) {
        match self.open.pop() {
            Some(Open::List(items)) => self.put(Node::List(items)),
            Some(Open::Map(entries, None)) => self.put(Node::Map(entries)),
            _ => self.refused = true,
        }
    }
}

fn is_null(text: &str, style: TScalarStyle) -> bool {
    style == TScalarStyle::Plain && matches!(text, "" | "~" | "null" | "Null" | "NULL")
}

impl MarkedEventReceiver for Reader {
    fn on_event(&mut self, event: Event, _mark: Marker) {
        match event {
            Event::DocumentStart => self.documents += 1,
            // An anchor has an identifier other than 0.
            Event::Alias(_) => self.refused = true,
            Event::Scalar(text, style, anchor, tag) => {
                self.refused |= anchor != 0 || tag.is_some();
                let node = if is_null(&text, style) {
                    Node::Null
                } else {
                    Node::Text(text)
                };
                self.put(node);
            }
            Event::SequenceStart(anchor, tag) => {
                self.refused |= anchor != 0 || tag.is_some();
                self.open(Open::List(Vec::new()));
            }
            Event::MappingStart(anchor, tag) => {
                self.refused |= anchor != 0 || tag.is_some();
                self.open(Open::Map(Vec::new(), None));
            }
            Event::SequenceEnd | Event::MappingEnd => self.close(),
            Event::Nothing | Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
        }
    }
}

pub(crate) fn read(file: &str) -> Result<Node, Refusal> {
    let refused = Refusal::top(Rule::YamlNotPermitted);
    let mut reader = Reader::default();
    Parser::new_from_str(file)
        .load(&mut reader, true)
        .map_err(|_| refused)?;
    match reader.root {
        Some(root) if !reader.refused && reader.documents == 1 && reader.open.is_empty() => {
            Ok(root)
        }
        _ => Err(refused),
    }
}
