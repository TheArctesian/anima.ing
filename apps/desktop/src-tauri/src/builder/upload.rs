//! The updater file's SHA-256 must be the bundle record's (D-22), so the bytes uploaded are the
//! bytes the bundle step wrote; the hub signs the bytes it reads back (R-117).
//! The web view chooses no byte it sends. A presigned URL is a credential: it never reaches a line, an error or a log (LOG-007,
//! BLD-049).

use std::{fmt::Write as _, io::Read, path::Path, time::Duration};

use reqwest::{Url, header};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tauri::{Manager as _, Runtime, State, Webview};

use super::{
    artifact, bundled::Bundle, clone_folder, error::BuilderError, file_name, from_main, quiet,
    recorded,
};

const SUMS: &str = "SHA256SUMS";
/// The largest file the hub takes: K1 `Int` sizes (R-108).
const MAX_FILE_BYTES: u64 = i32::MAX as u64;
/// How long one upload may take: the URL's own life (BLD-060, D-28).
const UPLOAD_LIMIT: Duration = Duration::from_mins(15);
/// `http` to this computer only in a test or a debug build, for a stand-in bucket.
const LOOPBACK_HTTP: bool = cfg!(any(test, debug_assertions));

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseFile {
    pub name: String,
    pub sha256: String,
    pub size: u64,
}

/// No signature: the hub signs (R-117).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseFiles {
    pub files: Vec<ReleaseFile>,
    pub updater: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadHeader {
    pub name: String,
    pub value: String,
}

/// No `Debug`: the URL is a credential.
#[derive(Clone, Deserialize)]
pub struct Upload {
    pub name: String,
    pub url: String,
    pub headers: Vec<UploadHeader>,
}

#[derive(Clone, Deserialize)]
pub struct UploadJob {
    target: String,
    version: String,
    upload: Upload,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Put {
    pub url: Url,
    pub cache_control: String,
    pub content_type: Option<String>,
}

pub fn sendable(url: &Url, loopback_http: bool) -> bool {
    let loopback = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) => name == "localhost",
        None => false,
    };
    let scheme = url.scheme() == "https" || (url.scheme() == "http" && loopback && loopback_http);
    scheme && url.username().is_empty() && url.password().is_none() && url.host().is_some()
}

/// Only a URL whose path ends with the job's `object`, and only the `Content-Length` (equal to
/// `size`),
/// `Cache-Control` and `Content-Type` headers, each once (BLD-060, BLD-061).
pub fn checked(upload: &Upload, size: usize, object: &str) -> Result<Put, BuilderError> {
    let url = Url::parse(&upload.url).map_err(|_| BuilderError::Upload)?;
    if !sendable(&url, LOOPBACK_HTTP) || !url.path().ends_with(&format!("/{object}")) {
        return Err(BuilderError::Upload);
    }
    let (mut length, mut cache_control, mut content_type) = (None, None, None);
    for header in &upload.headers {
        let slot = match header.name.to_ascii_lowercase().as_str() {
            "content-length" => &mut length,
            "cache-control" => &mut cache_control,
            "content-type" => &mut content_type,
            _ => return Err(BuilderError::Upload),
        };
        if slot.replace(header.value.clone()).is_some() {
            return Err(BuilderError::Upload);
        }
    }
    if length != Some(size.to_string()) {
        return Err(BuilderError::Upload);
    }
    let cache_control = cache_control.ok_or(BuilderError::Upload)?;
    Ok(Put {
        url,
        cache_control,
        content_type,
    })
}

pub fn sums(files: &[ReleaseFile]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .map(|file| format!("{}  {}\n", file.sha256, file.name))
        .collect();
    lines.sort_unstable();
    lines.concat()
}

fn read_own(clone: &Path, path: &Path) -> Result<Vec<u8>, BuilderError> {
    let file = artifact::open_own(clone, path)?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| BuilderError::Bundle)?;
    (bytes.len() as u64 <= MAX_FILE_BYTES)
        .then_some(bytes)
        .ok_or(BuilderError::Bundle)
}

pub fn sha256_of(mut file: impl Read) -> Result<[u8; 32], BuilderError> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        match file.read(&mut buffer).map_err(|_| BuilderError::Bundle)? {
            0 => return Ok(hash.finalize().into()),
            read => hash.update(&buffer[..read]),
        }
    }
}

/// Only while their SHA-256 is still the bundle record's (D-22).
pub fn bound_updater(clone: &Path, bundle: &Bundle) -> Result<Vec<u8>, BuilderError> {
    let bytes = read_own(clone, &bundle.path)?;
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    (sha256 == bundle.sha256)
        .then_some(bytes)
        .ok_or(BuilderError::Bundle)
}

fn described(name: String, bytes: &[u8]) -> ReleaseFile {
    let sha256 = Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        });
    ReleaseFile {
        name,
        sha256,
        size: bytes.len() as u64,
    }
}

fn bundle_files(clone: &Path, bundle: &Bundle) -> Result<Vec<ReleaseFile>, BuilderError> {
    let updater = described(file_name(&bundle.path)?, &bound_updater(clone, bundle)?);
    let mut files = vec![updater];
    if let Some(dmg) = artifact::dmg_file(clone, bundle.target, &bundle.version) {
        files.push(described(file_name(&dmg)?, &read_own(clone, &dmg)?));
    }
    Ok(files)
}

fn file_bytes(clone: &Path, bundle: &Bundle, name: &str) -> Result<Vec<u8>, BuilderError> {
    if name == SUMS {
        return Ok(sums(&bundle_files(clone, bundle)?).into_bytes());
    }
    if name == file_name(&bundle.path)? {
        return bound_updater(clone, bundle);
    }
    let dmg = artifact::dmg_file(clone, bundle.target, &bundle.version);
    match dmg {
        Some(path) if name == file_name(&path)? => read_own(clone, &path),
        _ => Err(BuilderError::Upload),
    }
}

/// `SHA256SUMS` comes last (BLD-063, UPD-010).
#[tauri::command]
pub async fn builder_release_files<R: Runtime>(
    webview: Webview<R>,
    running: State<'_, super::Running>,
    target: String,
    version: String,
) -> Result<ReleaseFiles, BuilderError> {
    from_main(&webview)?;
    let app = webview.app_handle().clone();
    let bundle = recorded(&app, &target, &version)?;
    let _claim = quiet(&running)?;
    let clone = clone_folder(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let updater = file_name(&bundle.path)?;
        let mut files = bundle_files(&clone, &bundle)?;
        let sums = sums(&files);
        files.push(described(SUMS.to_owned(), sums.as_bytes()));
        Ok(ReleaseFiles { files, updater })
    })
    .await
    .map_err(|_| BuilderError::Failed)?
}

/// Sends only the headers the hub fixed, and follows no redirect (BLD-060, BLD-063).
#[tauri::command]
pub async fn builder_upload<R: Runtime>(
    webview: Webview<R>,
    running: State<'_, super::Running>,
    job: UploadJob,
) -> Result<(), BuilderError> {
    from_main(&webview)?;
    let app = webview.app_handle().clone();
    let bundle = recorded(&app, &job.target, &job.version)?;
    let _claim = quiet(&running)?;
    let clone = clone_folder(&app)?;
    let name = job.upload.name.clone();
    let object = format!(
        "{}/{}/{name}",
        bundle.target.folder(),
        bundle.version.as_str()
    );
    let body = tauri::async_runtime::spawn_blocking(move || file_bytes(&clone, &bundle, &name))
        .await
        .map_err(|_| BuilderError::Failed)??;
    let put = checked(&job.upload, body.len(), &object)?;
    send(put, body).await
}

pub(super) async fn send(put: Put, body: Vec<u8>) -> Result<(), BuilderError> {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(UPLOAD_LIMIT)
        .build()
        .map_err(|_| BuilderError::Upload)?;
    let mut request = http
        .put(put.url)
        .header(header::CACHE_CONTROL, put.cache_control);
    if let Some(content_type) = put.content_type {
        request = request.header(header::CONTENT_TYPE, content_type);
    }
    let response = request
        .body(body)
        .send()
        .await
        .map_err(|_| BuilderError::Upload)?;
    response
        .status()
        .is_success()
        .then_some(())
        .ok_or(BuilderError::Upload)
}
