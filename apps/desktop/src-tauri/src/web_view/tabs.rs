//! The lock is held only to copy in or out, never across a call to the web view or an `await`.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use super::{Fingerprint, Pin, PinnedOrigin, WebUrl, WebViewError};

/// Each is a `WebKit` process.
pub const TABS_MAX: usize = 32;

const TAB_ID_MAX: usize = 128;

/// It becomes part of the web view and window labels, so no other character may reach a label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TabId(String);

impl TabId {
    pub fn parse(text: &str) -> Result<Self, WebViewError> {
        let allowed = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-';
        if (1..=TAB_ID_MAX).contains(&text.len()) && text.bytes().all(allowed) {
            Ok(Self(text.to_owned()))
        } else {
            Err(WebViewError::TabId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// No capability names this label, so the web view has no IPC (D-3).
    pub fn view_label(&self) -> String {
        format!("web-{}", self.0)
    }

    pub fn window_label(&self) -> String {
        format!("pop-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub url: WebUrl,
    pub origin: Option<PinnedOrigin>,
    pub pin: Option<Pin>,
    pub preview_endpoint: Option<uuid::Uuid>,
    pub popped_out: bool,
}

/// A tab opened again under the same ID has a new generation, so work begun for the old one cannot
/// touch it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(u64);

#[derive(Debug, Default)]
struct Open {
    tabs: HashMap<TabId, (Generation, Tab)>,
    next: u64,
}

#[derive(Debug, Default)]
pub struct Tabs(Mutex<Open>);

impl Tabs {
    /// A poisoned lock still holds a consistent map: every change is one insert, update or remove.
    fn open(&self) -> MutexGuard<'_, Open> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn reserve(&self, id: TabId, tab: Tab) -> Result<Generation, WebViewError> {
        let mut open = self.open();
        if open.tabs.contains_key(&id) {
            return Err(WebViewError::TabOpen);
        }
        if open.tabs.len() >= TABS_MAX {
            return Err(WebViewError::TabLimit);
        }
        let generation = Generation(open.next);
        open.next += 1;
        open.tabs.insert(id, (generation, tab));
        Ok(generation)
    }

    pub fn get(&self, id: &TabId) -> Result<Tab, WebViewError> {
        self.open()
            .tabs
            .get(id)
            .map(|(_, tab)| tab.clone())
            .ok_or(WebViewError::UnknownTab)
    }

    pub fn holds(&self, id: &TabId, generation: Generation) -> bool {
        self.open()
            .tabs
            .get(id)
            .is_some_and(|(held, _)| *held == generation)
    }

    pub fn set_pin(&self, id: &TabId, fingerprint: Fingerprint) -> Result<Tab, WebViewError> {
        let mut open = self.open();
        let (_, tab) = open.tabs.get_mut(id).ok_or(WebViewError::UnknownTab)?;
        if tab.preview_endpoint.is_some() {
            return Err(WebViewError::Arguments);
        }
        let origin = tab.origin.clone().ok_or(WebViewError::PinScheme)?;
        tab.pin = Some(Pin {
            origin,
            fingerprint,
        });
        Ok(tab.clone())
    }

    pub fn pop_out(&self, id: &TabId) -> Result<Option<Generation>, WebViewError> {
        let mut open = self.open();
        let (generation, tab) = open.tabs.get_mut(id).ok_or(WebViewError::UnknownTab)?;
        Ok((!std::mem::replace(&mut tab.popped_out, true)).then_some(*generation))
    }

    pub fn undo_pop_out(&self, id: &TabId, generation: Generation) {
        if let Some((held, tab)) = self.open().tabs.get_mut(id)
            && *held == generation
        {
            tab.popped_out = false;
        }
    }

    pub fn remove(&self, id: &TabId) -> Option<Tab> {
        self.open().tabs.remove(id).map(|(_, tab)| tab)
    }

    pub fn release(&self, id: &TabId, generation: Generation) -> Option<Tab> {
        let mut open = self.open();
        let held = open
            .tabs
            .get(id)
            .is_some_and(|(held, _)| *held == generation);
        held.then(|| open.tabs.remove(id))
            .flatten()
            .map(|(_, tab)| tab)
    }
}
