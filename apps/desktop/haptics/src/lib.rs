//! Haptic feedback on the iPhone (mobile plan M4-4, MOT-050). The web view of iOS cannot vibrate, so the shell plays
//! the feedback patterns of iOS. iOS plays nothing when System Haptics is off in Settings; Android has no native part.

use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_haptics);

/// A feedback pattern of the system, named for what it means, not for how it feels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Feel {
    /// A value moved by one step under the finger.
    Selection,
    /// A task ended well.
    Success,
    /// A task failed.
    Error,
}

/// The feedback generators of iOS.
#[cfg(target_os = "ios")]
pub struct Haptics<R: Runtime>(tauri::plugin::PluginHandle<R>);

/// iOS did not answer the call.
#[cfg(target_os = "ios")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotPlayed;

#[cfg(target_os = "ios")]
#[derive(serde::Serialize)]
struct Play {
    feel: Feel,
}

#[cfg(target_os = "ios")]
impl<R: Runtime> Haptics<R> {
    /// Plays `feel` on the main thread of iOS and returns when it is given to the generator.
    ///
    /// # Errors
    /// `NotPlayed` when the native plugin did not answer.
    pub async fn play(&self, feel: Feel) -> Result<(), NotPlayed> {
        self.0
            .run_mobile_plugin_async::<()>("play", Play { feel })
            .await
            .map_err(|_| NotPlayed)
    }
}

/// The plugin. Register it once, in the mobile application.
#[must_use]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("haptics")
        .setup(|app, api| {
            #[cfg(target_os = "ios")]
            {
                use tauri::Manager;
                app.manage(Haptics(api.register_ios_plugin(init_plugin_haptics)?));
            }
            #[cfg(not(target_os = "ios"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}
