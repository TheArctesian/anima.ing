//! Linux (WebKitGTK): not compiled or run by the builder (macOS only); the operator runs it.
//! The allowance is per host and web context, and a system-trusted certificate never signals, so D-4 is not enforced here.

use tauri::Url;
use tauri::webview::PlatformWebview;
use webkit2gtk::glib::{ByteArray, prelude::ObjectExt};
use webkit2gtk::{WebContextExt, WebViewExt};

use super::TrustHook;
use crate::pin::{Fingerprint, Presented, Verdict};

/// Runs on the GTK main thread (`with_webview`).
pub fn install(platform: &PlatformWebview, hook: TrustHook) -> Result<String, String> {
    let view = platform.inner();
    let label = hook.label().to_owned();
    view.connect_load_failed_with_tls_errors(move |view, uri, certificate, _errors| {
        let Some((host, port)) = origin(uri) else {
            return true;
        };
        let der: ByteArray = certificate.property("certificate");
        // The signal fires only for certificates the system did not trust.
        let presented = Presented {
            host: host.clone(),
            port,
            sha256: Fingerprint::of(&der),
            system_trusted: false,
        };
        if hook.judge(&presented) == Verdict::Accept {
            if let Some(context) = view.context() {
                context.allow_tls_certificate_for_host(certificate, &host);
                view.load_uri(uri);
            }
        }
        // Handled: WebKit shows no error page of its own.
        true
    });
    Ok(format!("{label}: load-failed-with-tls-errors connected"))
}

fn origin(uri: &str) -> Option<(String, u16)> {
    let url = Url::parse(uri).ok()?;
    Some((url.host_str()?.to_owned(), url.port_or_known_default()?))
}
