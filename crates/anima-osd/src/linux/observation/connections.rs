//! Aggregate observations of local graphical TCP transports.
use super::status::read_text;
use serde::Serialize;
use std::{io, path::Path};

#[derive(Default, Serialize)]
pub(super) struct Counts {
    pub rdp: u32,
    pub vnc: u32,
}

pub(super) fn observe() -> Option<Counts> {
    let mut counts = Counts::default();
    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        match read_text(Path::new(path), 2 * 1024 * 1024) {
            Ok(text) => count(&mut counts, &text)?,
            Err(error) if path.ends_with("tcp6") && error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    Some(counts)
}

fn count(counts: &mut Counts, text: &str) -> Option<()> {
    let mut lines = text.lines();
    if !lines.next()?.contains("local_address") {
        return None;
    }
    for line in lines {
        let mut fields = line.split_whitespace();
        let (_, port) = fields.nth(1)?.rsplit_once(':')?;
        let port = u16::from_str_radix(port, 16).ok()?;
        if fields.nth(1)? == "01" {
            match port {
                3389 => counts.rdp += 1,
                5900 => counts.vnc += 1,
                _ => (),
            }
        }
    }
    Some(())
}
