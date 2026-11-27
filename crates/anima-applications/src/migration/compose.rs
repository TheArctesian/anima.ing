//! Translate an operator's legacy manifest to Hosting's Compose input.
mod binds;
mod document;
mod fields;
mod ports;

use std::{
    fs::File,
    io::{self, Read as _},
    os::unix::fs::MetadataExt as _,
    path::Path,
};

use compose_rules::{LARGEST_ENVIRONMENT, LARGEST_FILE, Node, Variables};
use rustix::fs::{Mode, OFlags, open};
use zeroize::Zeroizing;

use crate::execution::files::Store;

use super::{Error, Manifest, local_root};
use binds::Binds;

pub struct ComposeInputs<'a> {
    pub mapping: &'a Path,
    pub environment: Option<&'a Path>,
}

/// Prepare a reviewable Compose document from stdin and the exact installed import.
///
/// # Errors
/// Invalid import, bind mapping, unsafe input files, or unsupported Compose configuration.
pub fn prepare_compose(
    root: &Path,
    ids: (&str, &str),
    inputs: &ComposeInputs<'_>,
) -> Result<String, Error> {
    let root_text = local_root(root, ids)?;
    let store = Store::observe(root)?;
    let application = format!("{root_text}/{}", ids.0);
    let manifest = Manifest::read(
        &store,
        &format!("{application}/migration.json"),
        (ids.0, Some(ids.1)),
    )?;
    for bind in &manifest.binds {
        store.existing_bind(&format!("{application}/binds/{bind}"))?;
    }
    let mapping = file(inputs.mapping, LARGEST_ENVIRONMENT, false)?;
    let legacy = bounded(io::stdin().lock(), LARGEST_FILE)?;
    let environment = inputs
        .environment
        .map(|path| file(path, LARGEST_ENVIRONMENT, true))
        .transpose()?
        .unwrap_or_default();
    translate(&manifest.binds, &mapping, &legacy, &environment)
}

fn file(path: &Path, limit: usize, private: bool) -> Result<Zeroizing<String>, Error> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let handle = File::from(open(path, flags, Mode::empty()).map_err(|_| Error::ComposeInput)?);
    let metadata = handle.metadata().map_err(|_| Error::ComposeInput)?;
    if !metadata.is_file() || (private && (metadata.uid() != 0 || metadata.mode() & 0o077 != 0)) {
        return Err(Error::ComposeInput);
    }
    bounded(handle, limit)
}

fn bounded(input: impl io::Read, limit: usize) -> Result<Zeroizing<String>, Error> {
    let mut text = Zeroizing::new(String::new());
    input
        .take(limit as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| Error::ComposeInput)?;
    if text.len() > limit {
        return Err(Error::ComposeInput);
    }
    Ok(text)
}

fn translate(
    installed: &[String],
    mapping: &str,
    legacy: &str,
    environment: &str,
) -> Result<String, Error> {
    let mut binds = Binds::read(mapping, installed)?;
    let variables = Variables::parse(environment)?;
    let Node::Map(mut document) = compose_rules::read_document(legacy)? else {
        return Err(Error::ComposeShape);
    };
    let volumes = binds::declared(&document)?;
    let Some(Node::Map(services)) = member(&mut document, "services") else {
        return Err(Error::ComposeShape);
    };
    for (_, service) in services {
        let Node::Map(attributes) = service else {
            return Err(Error::ComposeShape);
        };
        if let Some(mounts) = member(attributes, "volumes") {
            binds.rewrite(mounts, &volumes, &variables)?;
        }
        if let Some(ports) = member(attributes, "ports") {
            ports::rewrite(ports, &variables)?;
        }
    }
    binds.require_all_used(&volumes)?;
    document
        .retain(|(key, value)| key != "volumes" && !(key == "version" && value.text().is_some()));
    let output = document::write(&Node::Map(document))?;
    compose_rules::check_portable(&output, environment)?;
    Ok(output)
}

fn member<'a>(attributes: &'a mut [(String, Node)], key: &str) -> Option<&'a mut Node> {
    attributes
        .iter_mut()
        .find(|(name, _)| name == key)
        .map(|(_, node)| node)
}
