//! The limits of the files that go with a prompt (TH111), and the checks of a file that need no
//! I/O: its name, its kind and its size. The runner checks each again; a client checks them
//! first, so that the person hears of a limit before the bytes go.

/// The most bytes of one image.
pub const MOST_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// The most bytes of one file that is not an image.
pub const MOST_FILE_BYTES: u64 = 50 * 1024 * 1024;
/// The most files with one prompt.
pub const MOST_ATTACHMENTS: usize = 100;
/// The most bytes of the images of one prompt together.
pub const MOST_IMAGE_BYTES_IN_PROMPT: u64 = 80 * 1024 * 1024;
/// The most bytes of one part, before base64: a part in base64, in a frame, fits in
/// [`super::MAX_FRAME`] with room.
pub const MOST_PART_BYTES: usize = 2 * 1024 * 1024;
/// The most bytes of a name, after it was made safe.
pub const MOST_NAME_BYTES: usize = 120;

/// The kinds of image that both programs take as an image (measured on Claude Code 2.1.311 and
/// Codex 0.170, 2026-11-21).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageKind {
    /// The kind that a MIME type names, when it is one of the four.
    #[must_use]
    pub fn of_mime(mime: &str) -> Option<Self> {
        match mime.trim().to_ascii_lowercase().as_str() {
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }

    /// The kind that the first bytes of a file say.
    #[must_use]
    pub fn of_bytes(first: &[u8]) -> Option<Self> {
        if first.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if first.starts_with(&[0xff, 0xd8, 0xff]) {
            Some(Self::Jpeg)
        } else if first.starts_with(b"GIF87a") || first.starts_with(b"GIF89a") {
            Some(Self::Gif)
        } else if first.len() >= 12 && first.starts_with(b"RIFF") && &first[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else {
            None
        }
    }

    /// The MIME type that the programs take.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

/// True when the first bytes are those of a PDF.
#[must_use]
pub fn is_pdf(first: &[u8]) -> bool {
    first.starts_with(b"%PDF-")
}

/// The most bytes of a file of this MIME type.
#[must_use]
pub fn most_bytes(mime: &str) -> u64 {
    if ImageKind::of_mime(mime).is_some() {
        MOST_IMAGE_BYTES
    } else {
        MOST_FILE_BYTES
    }
}

/// A name that is safe as the last part of a path and in the words of a prompt: the last part of
/// the name only, with no control character, no leading dot, at most [`MOST_NAME_BYTES`].
#[must_use]
pub fn safe_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut safe = String::new();
    for char in last.chars().filter(|char| !char.is_control()) {
        if safe.len() + char.len_utf8() > MOST_NAME_BYTES {
            break;
        }
        safe.push(char);
    }
    let safe = safe.trim().trim_start_matches('.').trim();
    if safe.is_empty() {
        "attachment".to_owned()
    } else {
        safe.to_owned()
    }
}

/// For serde: a count of zero is left out.
#[must_use]
pub const fn is_zero(count: &u32) -> bool {
    *count == 0
}

#[cfg(test)]
mod tests {
    use super::{ImageKind, safe_name};

    #[test]
    fn th111_a_name_cannot_leave_its_folder_or_hide() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("..\\x\\.env"), "env");
        assert_eq!(safe_name("a\u{0}b\nc.png"), "abc.png");
        assert_eq!(safe_name(".."), "attachment");
        assert_eq!(safe_name("/"), "attachment");
        assert!(safe_name(&"é".repeat(200)).len() <= super::MOST_NAME_BYTES);
    }

    #[test]
    fn th111_the_kind_of_an_image_comes_from_its_bytes() {
        assert_eq!(
            ImageKind::of_bytes(b"\x89PNG\r\n\x1a\nxx"),
            Some(ImageKind::Png)
        );
        assert_eq!(
            ImageKind::of_bytes(b"RIFF\0\0\0\0WEBPVP8 "),
            Some(ImageKind::Webp)
        );
        assert_eq!(ImageKind::of_bytes(b"<svg>"), None);
        assert_eq!(ImageKind::of_mime("IMAGE/JPG"), Some(ImageKind::Jpeg));
        assert_eq!(ImageKind::of_mime("image/heic"), None);
    }
}
