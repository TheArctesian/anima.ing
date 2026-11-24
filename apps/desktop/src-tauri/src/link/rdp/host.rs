use std::{
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
    sync::Mutex,
};

use anima_rdp_host::{Host, Identity};
use rand::RngExt as _;
use security_framework::passwords::{get_generic_password, set_generic_password};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons};
use zeroize::Zeroizing;

use super::{Changing, LinkError, PROGRAM, access, folders, open_ports};

const PORT: u16 = 3389;
const SOCKET: &str = "rdp.sock";
const SERVICE: &str = "ing.anima.desktop.rdp-host";
const ACCOUNT: &str = "authentication";
const NOT_FOUND: i32 = -25300;

static HOST: Mutex<Option<Host>> = Mutex::new(None);

#[derive(Deserialize, Serialize)]
struct Record {
    username: String,
    password: String,
    certificate: Vec<u8>,
    private_key: Vec<u8>,
    public_key: Vec<u8>,
}

impl Drop for Record {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.password.zeroize();
        self.private_key.zeroize();
    }
}

impl Record {
    fn read() -> Result<Option<Self>, &'static str> {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(found) => serde_json::from_slice(&Zeroizing::new(found))
                .map(Some)
                .map_err(|_| "The saved RDP identity could not be read."),
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            Err(_) => Err("Anima could not read the RDP identity from Keychain."),
        }
    }

    fn create() -> Result<Self, &'static str> {
        let username =
            whoami::username().map_err(|_| "The macOS account name could not be read.")?;
        let random: [u8; 24] = rand::rng().random();
        let password = {
            use base64::Engine as _;
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random)
        };
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["localhost".into()])
                .map_err(|_| "The RDP certificate could not be created.")?;
        let record = Self {
            username,
            password,
            certificate: cert.der().to_vec(),
            private_key: signing_key.serialize_der(),
            public_key: signing_key.public_key_raw().to_vec(),
        };
        let encoded = Zeroizing::new(
            serde_json::to_vec(&record).map_err(|_| "The RDP identity could not be saved.")?,
        );
        set_generic_password(SERVICE, ACCOUNT, &encoded)
            .map_err(|_| "Anima could not save the RDP identity in Keychain.")?;
        Ok(record)
    }

    fn identity(&self) -> Identity {
        Identity {
            username: self.username.clone(),
            password: Zeroizing::new(self.password.clone()),
            certificate: self.certificate.clone(),
            private_key: Zeroizing::new(self.private_key.clone()),
            public_key: self.public_key.clone(),
        }
    }
}

fn run(folder: &Path, record: &Record) -> Result<(), &'static str> {
    let mut host = HOST.lock().map_err(|_| "The RDP host is busy.")?;
    if host.as_ref().is_some_and(Host::running) {
        return Ok(());
    }
    *host = None;
    let running = Host::start(folder.join(SOCKET), record.identity())
        .map_err(|_| "The bundled RDP host could not start. Check permissions and port 3389.")?;
    *host = Some(running);
    Ok(())
}

pub fn running() -> bool {
    HOST.lock()
        .is_ok_and(|host| host.as_ref().is_some_and(Host::running))
}

pub fn close() {
    if let Ok(mut host) = HOST.lock() {
        *host = None;
    }
}

async fn consent<R: Runtime>(app: &AppHandle<R>) -> Result<(), &'static str> {
    let (reply, answer) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = reply.send(anima_rdp_host::request_permissions());
    })
    .map_err(|_| "Anima could not open the macOS permission prompts.")?;
    let permissions = answer
        .await
        .map_err(|_| "The macOS permission prompts did not finish.")?;
    let (text, pane) = match (permissions.screen, permissions.input) {
        (true, true) => return Ok(()),
        (false, _) => (
            "Allow Anima.ing in Screen & System Audio Recording, then quit and reopen Anima.",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        ),
        (true, false) => (
            "Allow Anima.ing in Accessibility, then try again.",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        ),
    };
    app.dialog()
        .message(text)
        .title("Remote Desktop permission")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open System Settings".into(),
            "Later".into(),
        ))
        .show(move |open| {
            if open {
                let _ = crate::os_program::open("/usr/bin/open", pane.as_ref());
            }
        });
    Err(text)
}

pub async fn prepare<R: Runtime>(app: &AppHandle<R>, folder: &Path) -> Result<(), &'static str> {
    if running() {
        return Ok(());
    }
    consent(app).await?;
    let folder = folder.to_path_buf();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let record = match Record::read()? {
            Some(record) => record,
            None => Record::create()?,
        };
        run(&folder, &record)?;
        show_credentials(&app, &record);
        Ok(())
    })
    .await
    .map_err(|_| "The bundled RDP host did not start.")?
}

fn show_credentials<R: Runtime>(app: &AppHandle<R>, record: &Record) {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    let fingerprint =
        Sha256::digest(&record.certificate)
            .iter()
            .fold(String::new(), |mut text, byte| {
                if !text.is_empty() {
                    text.push(':');
                }
                let _ = write!(text, "{byte:02X}");
                text
            });
    let password = Zeroizing::new(record.password.clone());
    app.dialog()
        .message(format!(
            "User name: {}\nCertificate SHA-256: {}\n\nTest the RDP connection and compare this certificate fingerprint. Use the dedicated password in the RDP Password field.",
            record.username, fingerprint,
        ))
        .title("Anima.ing RDP credentials")
        .buttons(MessageDialogButtons::OkCancelCustom("Copy RDP password".into(), "Done".into()))
        .show(move |copy| {
            if copy {
                let _ = copy_password(&password);
            }
        });
}

fn copy_password(password: &str) -> std::io::Result<()> {
    let mut child = Command::new("/usr/bin/pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("clipboard input is unavailable"))
        .and_then(|mut input| input.write_all(password.as_bytes()));
    let status = child.wait()?;
    written?;
    if !status.success() {
        return Err(std::io::Error::other("the clipboard operation failed"));
    }
    Ok(())
}

pub fn restore(changing: &Changing) {
    let Ok(claim) = changing.claim() else { return };
    std::mem::drop(tauri::async_runtime::spawn_blocking(move || {
        let _claim = claim;
        let Ok(folder) = folders().map(|folders| folders.device) else {
            return;
        };
        let Some(program) = crate::carried::program(PROGRAM) else {
            return;
        };
        let Ok(open) = open_ports::ask(&program, &folder, None) else {
            return;
        };
        if open.open.contains(&PORT)
            && let Ok(Some(record)) = Record::read()
        {
            let _ = run(&folder, &record);
        }
    }));
}

pub fn stop(changing: &Changing) -> Result<(), LinkError> {
    let _claim = changing.claim()?;
    close();
    let folder = folders()?.device;
    let program = crate::carried::program(PROGRAM).ok_or(LinkError::NotCarried)?;
    open_ports::ask(&program, &folder, Some((PORT, false)))?;
    access::remember(&folder, PORT, false)
}
