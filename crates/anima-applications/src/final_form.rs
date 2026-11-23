//! The final form of a project: the one file that Compose reads (K10 Section 6.4.4, step 5).

mod declared;

use compose_rules::{Checked, Node, Rule, forms::truth};

use crate::{
    Pulled, Refusal,
    forms::is_digest,
    json::{self, Json},
    settings::OwnLimits,
};

#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    /// The name of the project: `anima-` and the 32 hexadecimal digits of the application.
    pub project: &'a str,
    pub binds: &'a str,
    pub own: &'a OwnLimits,
    pub open_to_network: bool,
    pub digests: &'a [Pulled],
}

/// Its text holds values of the environment file, so `{:?}` shows only its length.
#[derive(Clone, PartialEq, Eq)]
pub struct FinalForm {
    pub text: String,
    pub binds: Vec<String>,
    /// A volume or a bind mount holds data that `application_back` does not bring back.
    pub holds_volumes: bool,
}

impl std::fmt::Debug for FinalForm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FinalForm")
            .field("text_bytes", &self.text.len())
            .field("binds", &self.binds)
            .field("holds_volumes", &self.holds_volumes)
            .finish()
    }
}

const fn refused(rule: Rule, service: Option<usize>) -> Refusal {
    Refusal::FileNotPermitted { rule, service }
}

fn holds_nul(node: &Node) -> bool {
    match node {
        Node::Null => false,
        Node::Text(text) => text.contains('\0'),
        Node::List(items) => items.iter().any(holds_nul),
        Node::Map(entries) => entries
            .iter()
            .any(|(key, value)| key.contains('\0') || holds_nul(value)),
    }
}

/// By digest, so the engine never starts an image on the machine for a different reason (SF-041).
pub(crate) fn image_by_digest(image: &str, digest: &str) -> String {
    let (folders, last) = image
        .rsplit_once('/')
        .map_or(("", image), |(folders, last)| (folders, last));
    let name = last.split(['@', ':']).next().unwrap_or(last);
    let separator = if folders.is_empty() { "" } else { "/" };
    format!("{folders}{separator}{name}@{digest}")
}

fn digest_of<'a>(frame: &Frame<'a>, name: &str, image: &str) -> Option<&'a str> {
    frame
        .digests
        .iter()
        .find(|pulled| pulled.service == name && pulled.image == image)
        .map(|pulled| pulled.digest.as_str())
        .filter(|digest| is_digest(digest))
        .filter(|digest| {
            image
                .split_once('@')
                .is_none_or(|(_, named)| named == *digest)
        })
}

fn services(document: &Node, frame: &Frame<'_>) -> Result<Json, Refusal> {
    let mut members = Vec::new();
    let entries = document
        .get("services")
        .and_then(Node::entries)
        .unwrap_or(&[]);
    for (position, (name, node)) in entries.iter().enumerate() {
        let image = node.get("image").and_then(Node::text);
        let image = image.ok_or(refused(Rule::ImageNotPermitted, Some(position)))?;
        let digest = digest_of(frame, name, image).ok_or(Refusal::NoDigest)?;
        let formed = declared::formed(node, image_by_digest(image, digest), frame)
            .map_err(|_| refused(Rule::ValueNotPermitted, Some(position)))?;
        members.push((name.clone(), formed));
    }
    Ok(Json::Record(members))
}

fn network(node: &Node) -> Result<Json, Refusal> {
    let mut members = Vec::new();
    if node.get("driver").and_then(Node::text).is_some() {
        members.push(("driver".to_owned(), Json::text("bridge")));
    }
    if let Some(internal) = node.get("internal").and_then(Node::text) {
        let internal = truth(internal).ok_or(refused(Rule::ValueNotPermitted, None))?;
        members.push(("internal".to_owned(), Json::Truth(internal)));
    }
    Ok(Json::Record(members))
}

fn named(
    document: &Node,
    key: &str,
    reader: impl Fn(&Node) -> Result<Json, Refusal>,
) -> Result<Option<(String, Json)>, Refusal> {
    let Some(entries) = document.get(key).and_then(Node::entries) else {
        return Ok(None);
    };
    let mut members = Vec::new();
    for (name, node) in entries {
        members.push((name.clone(), reader(node)?));
    }
    Ok(Some((key.to_owned(), Json::Record(members))))
}

fn binds(document: &Node) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let services = document
        .get("services")
        .and_then(Node::entries)
        .unwrap_or(&[]);
    for (_, node) in services {
        let Some(Node::List(mounts)) = node.get("volumes") else {
            continue;
        };
        for mount in mounts {
            let source = mount.get("source").and_then(Node::text);
            let name = source.and_then(compose_rules::forms::bind_name);
            let is_bind = mount.get("type").and_then(Node::text) == Some("bind");
            if let Some(name) =
                name.filter(|name| is_bind && !names.iter().any(|known| known == name))
            {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// # Errors
/// `no_digest` for an image with no digest in `frame`; `file_not_permitted` for a bad value.
pub fn formed(checked: &Checked, frame: &Frame<'_>) -> Result<FinalForm, Refusal> {
    let document = &checked.document;
    let entries = document
        .get("services")
        .and_then(Node::entries)
        .unwrap_or(&[]);
    if let Some(position) = entries
        .iter()
        .position(|(name, node)| name.contains('\0') || holds_nul(node))
    {
        return Err(refused(Rule::ValueNotPermitted, Some(position)));
    }
    if holds_nul(document) {
        return Err(refused(Rule::ValueNotPermitted, None));
    }
    let mut members = vec![
        ("name".to_owned(), Json::text(frame.project)),
        ("services".to_owned(), services(document, frame)?),
    ];
    let content = |node: &Node| {
        let content = node.get("content").and_then(Node::text);
        let content = content.ok_or(refused(Rule::ValueNotPermitted, None))?;
        Ok(Json::Record(vec![(
            "content".to_owned(),
            Json::text(content),
        )]))
    };
    members.extend(named(document, "volumes", |_| Ok(Json::empty()))?);
    members.extend(named(document, "networks", network)?);
    members.extend(named(document, "configs", content)?);
    let binds = binds(document);
    let volumes = document
        .get("volumes")
        .and_then(Node::entries)
        .unwrap_or(&[]);
    Ok(FinalForm {
        text: json::written(&Json::Record(members)),
        holds_volumes: !binds.is_empty() || !volumes.is_empty(),
        binds,
    })
}
