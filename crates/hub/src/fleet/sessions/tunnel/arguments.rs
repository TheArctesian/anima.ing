//! What the hub tells guacd about one RDP or VNC session (K4 Section 3, GW-012, GW-013,
//! RA-021, RA-033).
//!
//! The client supplies only the display and a zone-shaped time zone; every other value comes
//! from the hub, and names not set here stay empty.
//!
//! guacd reads a boolean as true only when it is exactly `true`; empty is false
//! (guacamole-server `guac_user_parse_args_boolean`).

use std::net::IpAddr;

use guac_protocol::{ArgValues, is_zone_name};

use crate::fleet::credentials::{PasswordKind, Secret};
use crate::fleet::devices::{Device, RdpSecurity, ResizeMethod, TransferPolicy};
use crate::fleet::{sessions::Desktop, trust::ServerCertificate};

use super::policy::Policy;

/// The time zone when the client's is not zone-shaped (sites have no zone, R-032).
const FALLBACK_ZONE: &str = "UTC";
const TRUE: &str = "true";
const SOUND: &[&str] = &["audio/L8", "audio/L16"];
const FINGERPRINT_HASH: &str = "sha256";

impl Desktop {
    /// The name the handshake's `select` sends (K4 Section 3 step 1).
    pub(super) const fn select(self) -> &'static str {
        match self {
            Self::Rdp => "rdp",
            Self::Vnc => "vnc",
        }
    }

    pub(in crate::fleet::sessions) const fn password(self) -> PasswordKind {
        match self {
            Self::Rdp => PasswordKind::Rdp,
            Self::Vnc => PasswordKind::Vnc,
        }
    }

    pub(super) const fn port(self, device: &Device) -> u16 {
        match self {
            Self::Rdp => device.rdp.port,
            Self::Vnc => device.vnc.port,
        }
    }
}

/// Holds no value (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum BadConnectValue {
    #[error("the stored {argument} is not UTF-8 text")]
    NotText { argument: &'static str },
}

pub(super) struct Sources<'a> {
    pub(super) device: &'a Device,
    /// The address the hub resolved and checked (R-032), so guacd never resolves the name again.
    pub(super) target: IpAddr,
    /// The device's port, or the hub's port that carries to it through the link (HMD-048).
    pub(super) port: u16,
    /// Without one guacd asks the client (GW-015).
    pub(super) password: Option<&'a Secret>,
    /// The independently accepted certificate for the selected protocol (SEC-007).
    pub(super) certificate: Option<&'a ServerCertificate>,
    /// `ANIMA_GUACD_DRIVE_ROOT`, a path inside the guacd container (RA-021).
    pub(super) drive_root: &'a str,
}

/// The `connect` values by guacd argument name for `desktop` (K4 Section 3).
///
/// # Errors
/// [`BadConnectValue::NotText`] when the stored password is not UTF-8.
pub(super) fn connect_values(
    policy: Policy,
    sources: &Sources<'_>,
) -> Result<ArgValues, BadConnectValue> {
    let device = sources.device;
    let mut values = ArgValues::new();
    values.set("hostname", sources.target.to_string());
    values.set("port", sources.port.to_string());
    if let Some(password) = sources.password {
        values.set("password", text("password", password)?);
    }
    clipboard(&mut values, policy.transfer);
    flag(&mut values, "read-only", policy.view_only);
    match policy.desktop {
        Desktop::Rdp => rdp(&mut values, sources, policy),
        Desktop::Vnc => {
            flag(&mut values, "disable-display-resize", policy.view_only);
            if let Some(certificate) = sources.certificate {
                values.set("cert-fingerprints", accepted_fingerprint(certificate));
            }
            if let Some(username) = &device.vnc.username {
                values.set("username", username.as_str());
            }
        }
    }
    Ok(values)
}

fn rdp(values: &mut ArgValues, sources: &Sources<'_>, policy: Policy) {
    let device = sources.device;
    let rdp = &device.rdp;
    values.set("username", rdp.username.as_str());
    if let Some(domain) = &rdp.domain {
        values.set("domain", domain.as_str());
    }
    values.set("security", security(rdp.security));
    flag(values, "ignore-cert", rdp.ignore_certificate);
    if let Some(certificate) = sources.certificate {
        values.set("cert-fingerprints", accepted_fingerprint(certificate));
    }
    if !policy.view_only {
        values.set("resize-method", resize_method(rdp.resize));
    }
    flag(values, "disable-audio", !rdp.audio_out);
    flag(values, "enable-audio-input", policy.microphone);
    flag(values, "disable-download", !policy.transfer.download);
    flag(values, "disable-upload", !policy.transfer.upload);
    if policy.shared_drive {
        flag(values, "enable-drive", true);
        values.set("drive-path", drive_path(sources.drive_root, device));
        flag(values, "create-drive-path", true);
    }
}

/// The relay is the first clipboard guard and these switches the second (RA-033).
fn clipboard(values: &mut ArgValues, transfer: TransferPolicy) {
    flag(values, "disable-copy", !transfer.copy);
    flag(values, "disable-paste", !transfer.paste);
}

/// The id is a hyphenated UUID, so the path cannot climb out of the root (RA-021).
pub(super) fn drive_path(root: &str, device: &Device) -> String {
    format!("{}/{}", root.trim_end_matches('/'), device.id)
}

/// `FreeRDP` reads `CertificateAcceptedFingerprints` as `<hash>:<colon-joined hex>` and
/// compares the hex case-insensitively.
pub(super) fn accepted_fingerprint(certificate: &ServerCertificate) -> String {
    format!("{FINGERPRINT_HASH}:{}", certificate.fingerprint_sha256)
}

/// `Any` becomes `chosen`, the TLS security the server picked: with `any` guacd may settle on
/// standard RDP security, which checks no certificate (SEC-007).
pub(super) const fn pinned_security(set: RdpSecurity, chosen: RdpSecurity) -> RdpSecurity {
    match set {
        RdpSecurity::Any => chosen,
        other => other,
    }
}

/// guacd's `security` text (guacamole-server `src/protocols/rdp/settings.c`).
const fn security(security: RdpSecurity) -> &'static str {
    match security {
        RdpSecurity::Any => "any",
        RdpSecurity::Nla => "nla",
        RdpSecurity::NlaExt => "nla-ext",
        RdpSecurity::Tls => "tls",
        RdpSecurity::Vmconnect => "vmconnect",
        RdpSecurity::Rdp => "rdp",
    }
}

const fn resize_method(resize: ResizeMethod) -> &'static str {
    match resize {
        ResizeMethod::DisplayUpdate => "display-update",
        ResizeMethod::Reconnect => "reconnect",
    }
}

fn flag(values: &mut ArgValues, name: &str, on: bool) {
    if on {
        values.set(name, TRUE);
    }
}

/// The client plays sound only from an RDP device that sends it (K4 Section 3).
pub(super) const fn audio(desktop: Desktop, device: &Device) -> &'static [&'static str] {
    match desktop {
        Desktop::Rdp if device.rdp.audio_out => SOUND,
        Desktop::Rdp | Desktop::Vnc => &[],
    }
}

/// The client's zone when it is zone-shaped, else UTC, which the handshake always accepts
/// (K4 Section 3, R-032 E-6).
pub(super) fn time_zone(requested: Option<&str>) -> String {
    requested
        .filter(|zone| is_zone_name(zone))
        .unwrap_or(FALLBACK_ZONE)
        .to_owned()
}

/// `ArgValues::set` copies the secret into a `String` that is not zeroed on drop
/// (guac-protocol, issue #1).
fn text<'a>(argument: &'static str, secret: &'a Secret) -> Result<&'a str, BadConnectValue> {
    std::str::from_utf8(secret.expose()).map_err(|_| BadConnectValue::NotText { argument })
}
