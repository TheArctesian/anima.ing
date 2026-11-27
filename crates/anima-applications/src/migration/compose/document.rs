use compose_rules::Node;
use serde::{Serialize, Serializer, ser::SerializeMap as _};

use super::Error;

pub(super) fn write(document: &Node) -> Result<String, Error> {
    serde_json::to_string_pretty(&Original(document)).map_err(|_| Error::ComposeOutput)
}

/// JSON also reads as YAML. Keep service order and leave interpolation tokens verbatim.
struct Original<'a>(&'a Node);

impl Serialize for Original<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Node::Null => serializer.serialize_none(),
            Node::Text(value) => serializer.serialize_str(value),
            Node::List(values) => serializer.collect_seq(values.iter().map(Original)),
            Node::Map(values) => {
                let mut map = serializer.serialize_map(Some(values.len()))?;
                for (key, value) in values {
                    map.serialize_entry(key, &Original(value))?;
                }
                map.end()
            }
        }
    }
}

pub(super) fn field(key: &str, value: &str) -> (String, Node) {
    (key.to_owned(), Node::Text(value.to_owned()))
}
