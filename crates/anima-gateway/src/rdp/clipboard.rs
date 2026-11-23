//! The clipboard channel of an RDP session, text only (RA-005), answered under the device's switches (RA-033).

use ironrdp_cliprdr::backend::CliprdrBackend;
use ironrdp_cliprdr::pdu::{
    ClipboardFormat, ClipboardFormatId, ClipboardGeneralCapabilityFlags, FileContentsRequest,
    FileContentsResponse, FormatDataRequest, FormatDataResponse, LockDataId,
};
use ironrdp_core::impl_as_any;

/// The longest clipboard text taken from the device, in bytes: as for VNC.
const LONGEST_COPY: usize = 1 << 20;
const MOST_NOTES: usize = 16;

pub(super) const TEXT: ClipboardFormatId = ClipboardFormatId::CF_UNICODETEXT;

#[derive(Debug, PartialEq, Eq)]
pub enum Said {
    /// The channel is ready, or the device wants to know what the client's clipboard holds.
    WhatIsThere,
    /// Something was copied on the device, and text is among its formats.
    CopiedText,
    /// It pastes, and wants the client's text.
    WantsText,
    Text(String),
}

#[derive(Debug, Default)]
pub struct ClipboardNotes {
    said: Vec<Said>,
}

impl ClipboardNotes {
    pub fn take(&mut self) -> Vec<Said> {
        std::mem::take(&mut self.said)
    }

    fn note(&mut self, said: Said) {
        if self.said.len() < MOST_NOTES {
            self.said.push(said);
        }
    }
}

impl_as_any!(ClipboardNotes);

impl CliprdrBackend for ClipboardNotes {
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "the signature is the trait's"
    )]
    fn temporary_directory(&self) -> &str {
        ""
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        ClipboardGeneralCapabilityFlags::empty()
    }

    fn on_ready(&mut self) {
        self.note(Said::WhatIsThere);
    }

    fn on_request_format_list(&mut self) {
        self.note(Said::WhatIsThere);
    }

    fn on_process_negotiated_capabilities(&mut self, _: ClipboardGeneralCapabilityFlags) {}

    fn on_remote_copy(&mut self, available_formats: &[ClipboardFormat]) {
        if available_formats.iter().any(|format| format.id() == TEXT) {
            self.note(Said::CopiedText);
        }
    }

    fn on_format_data_request(&mut self, _: FormatDataRequest) {
        self.note(Said::WantsText);
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        // UTF-16 takes at least two bytes a character, so a response over twice the limit is too long.
        if response.is_error() || response.data().len() > 2 * LONGEST_COPY {
            return;
        }
        if let Ok(text) = response.to_unicode_string()
            && text.len() <= LONGEST_COPY
        {
            self.note(Said::Text(text));
        }
    }

    fn on_file_contents_request(&mut self, _: FileContentsRequest) {}

    fn on_file_contents_response(&mut self, _: FileContentsResponse<'_>) {}

    fn on_lock(&mut self, _: LockDataId) {}

    fn on_unlock(&mut self, _: LockDataId) {}
}

/// The switches of a device for the clipboard (RA-033).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sharing {
    /// The device's clipboard may go to the client.
    pub copy: bool,
    /// The client's clipboard may go to the device.
    pub paste: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reply<'a> {
    /// Tell the device whether the client's clipboard holds text.
    Offer(bool),
    AskForText,
    /// Give the device the client's text; `None` when there is none for it.
    GiveText(Option<&'a str>),
    ToClient(String),
    Nothing,
}

/// The answer to `said` for the client's clipboard `text`; the hub's relay is the first guard.
#[must_use]
pub fn reply(said: Said, sharing: Sharing, text: Option<&str>) -> Reply<'_> {
    let text = text.filter(|_| sharing.paste);
    match said {
        Said::WhatIsThere => Reply::Offer(text.is_some()),
        Said::WantsText => Reply::GiveText(text),
        Said::CopiedText if sharing.copy => Reply::AskForText,
        Said::Text(copied) if sharing.copy => Reply::ToClient(copied),
        Said::CopiedText | Said::Text(_) => Reply::Nothing,
    }
}
