//! Keeps the Android web view above the on-screen keyboard (MOB-011, mobile plan M4-1).
//! The app draws edge to edge, so Android does not shorten the web view; iOS reports the keyboard itself.

use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

/// Register it once, in the mobile application.
#[must_use]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("keyboard")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            api.register_android_plugin("ing.anima.keyboard", "KeyboardPlugin")?;
            #[cfg(not(target_os = "android"))]
            let _ = api;
            Ok(())
        })
        .build()
}
