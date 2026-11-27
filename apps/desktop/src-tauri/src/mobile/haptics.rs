//! The haptic feedback of the iPhone app for the web view (mobile plan M4-4, MOT-050).

use tauri::Manager;
use tauri_plugin_haptics::{Feel, Haptics};

/// Plays one feedback pattern of iOS.
///
/// # Errors
/// When the native plugin did not answer.
#[tauri::command]
pub async fn haptic_play<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    feel: Feel,
) -> Result<(), &'static str> {
    app.state::<Haptics<R>>()
        .play(feel)
        .await
        .map_err(|_| "iOS did not play the haptic feedback.")
}
