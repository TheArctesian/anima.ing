//! Parse the producer's explicit tar dialect with independently bounded extensions.
use super::tree::{Attributes, Node, Tree, invalid};
use crate::linux::{Error, Result, workspace::handoff::project::digest::Output};
use agent_threads::managed::workspace::handoff::Receipt;
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read, Write},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    time::Instant,
};
use tar::{Entry, EntryType};

pub(super) fn unpack(
    input: impl Read,
    root: File,
    binding: (&Receipt, u64),
    deadline: Instant,
) -> Result<()> {
    let (receipt, quota) = binding;
    let mut input = Meter {
        source: input,
        digest: Output::new(io::sink(), receipt.bytes, deadline),
        bytes: 0,
    };
    let mut tree = Tree::new(root, quota);
    let entries = entries(&mut tree, &mut input, receipt.entries)?;
    if entries != receipt.entries || input.bytes.checked_add(512) != Some(receipt.bytes) {
        return Err(invalid());
    }
    let mut tail = [0; 512];
    input.read_exact(&mut tail)?;
    if tail != [0; 512] {
        return Err(invalid());
    }
    let (digest, bytes) = input.digest.finish()?;
    if bytes != receipt.bytes || digest != receipt.digest {
        return Err(Error::Refused(
            "The checkpoint archive does not match its receipt.",
        ));
    }
    tree.finish()
}

fn entries(tree: &mut Tree, input: impl Read, limit: usize) -> Result<usize> {
    let mut archive = tar::Archive::new(input);
    let mut extensions = Extensions::default();
    let mut count = None;
    for entry in archive.entries()?.raw(true) {
        let mut entry = entry?;
        if extensions.read(&mut entry)? {
            continue;
        }
        let node = extensions.node(&entry)?;
        if let Some(count) = count.as_mut() {
            if *count >= limit {
                return Err(invalid());
            }
            tree.add(&node, &mut entry)?;
            *count += 1;
        } else {
            if node.path.as_os_str().as_bytes() != b"." || node.kind != EntryType::Directory {
                return Err(invalid());
            }
            tree.root(node.attributes)?;
            count = Some(0);
        }
    }
    if extensions.path.is_some() || extensions.link.is_some() {
        return Err(invalid());
    }
    count.ok_or(invalid())
}

#[derive(Default)]
struct Extensions {
    path: Option<PathBuf>,
    link: Option<PathBuf>,
}

impl Extensions {
    fn read(&mut self, entry: &mut Entry<'_, impl Read>) -> Result<bool> {
        let target = match entry.header().entry_type() {
            EntryType::GNULongName => &mut self.path,
            EntryType::GNULongLink => &mut self.link,
            _ => return Ok(false),
        };
        if target.is_some() || !(2..=4096).contains(&entry.size()) {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if bytes.len() as u64 != entry.size() || bytes.pop() != Some(0) {
            return Err(invalid());
        }
        *target = Some(path(&bytes)?);
        Ok(true)
    }

    fn node(&mut self, entry: &Entry<'_, impl Read>) -> Result<Node> {
        let header = entry.header();
        let kind = header.entry_type();
        let mode = header.mode()?;
        let bytes = entry.size();
        let name =
            choose_path(self.path.take(), Some(entry.path_bytes().as_ref()))?.ok_or(invalid())?;
        let link = choose_path(self.link.take(), entry.link_name_bytes().as_deref())?;
        validate(kind, bytes, mode, link.is_some())?;
        if header.as_gnu().is_none() || header.uid()? != 1000 || header.gid()? != 1000 {
            return Err(invalid());
        }
        Ok(Node {
            kind,
            path: name,
            link,
            bytes,
            attributes: Attributes {
                mode,
                mtime: header.mtime()?,
            },
        })
    }
}

fn validate(kind: EntryType, bytes: u64, mode: u32, has_link: bool) -> Result<()> {
    if mode > 0o777 {
        return Err(invalid());
    }
    match kind {
        EntryType::Regular if !has_link => Ok(()),
        EntryType::Directory if !has_link && bytes == 0 => Ok(()),
        EntryType::Link | EntryType::Symlink if has_link && bytes == 0 => Ok(()),
        _ => Err(invalid()),
    }
}

fn choose_path(extended: Option<PathBuf>, inline: Option<&[u8]>) -> Result<Option<PathBuf>> {
    if let Some(path) = extended {
        Ok(Some(path))
    } else {
        inline.map(path).transpose()
    }
}

fn path(bytes: &[u8]) -> Result<PathBuf> {
    if bytes.is_empty() || bytes.len() >= 4096 || bytes.contains(&0) {
        return Err(invalid());
    }
    Ok(PathBuf::from(OsStr::from_bytes(bytes)))
}

struct Meter<R> {
    source: R,
    digest: Output<io::Sink>,
    bytes: u64,
}

impl<R: Read> Read for Meter<R> {
    fn read(&mut self, destination: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(destination)?;
        self.digest.write_all(&destination[..count])?;
        self.bytes += count as u64;
        Ok(count)
    }
}

#[cfg(test)]
mod tests;
