//! Native preview views have no application JavaScript bridge.
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_workspace_preview);

#[derive(serde::Serialize)]
pub struct Opening {
    pub id: String,
    pub action: String,
    pub token: String,
}
#[cfg(any(target_os = "ios", target_os = "android"))]
#[derive(serde::Serialize)]
struct Closing<'a> {
    id: &'a str,
}
#[cfg(any(target_os = "ios", target_os = "android"))]
pub struct Preview<R: Runtime>(tauri::plugin::PluginHandle<R>);
#[cfg(any(target_os = "ios", target_os = "android"))]
impl<R: Runtime> Preview<R> {
    pub async fn open(&self, opening: Opening) -> Result<(), &'static str> {
        self.0
            .run_mobile_plugin_async::<()>("open", opening)
            .await
            .map_err(|_| "The workspace preview could not open.")
    }
    pub async fn close(&self, id: &str) -> Result<(), &'static str> {
        self.0
            .run_mobile_plugin_async::<()>("close", Closing { id })
            .await
            .map_err(|_| "The workspace preview could not close.")
    }
}
#[must_use]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("workspace-preview")
        .setup(|app, api| {
            #[cfg(any(target_os = "ios", target_os = "android"))]
            {
                use tauri::Manager;
                #[cfg(target_os = "ios")]
                let handle = api.register_ios_plugin(init_plugin_workspace_preview)?;
                #[cfg(target_os = "android")]
                let handle = api.register_android_plugin(
                    "ing.anima.workspacepreview",
                    "WorkspacePreviewPlugin",
                )?;
                app.manage(Preview(handle));
            }
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}
