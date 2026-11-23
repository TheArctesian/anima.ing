//! The daemon's log as a file with a size bound (MET-019, R-312).
//! launchd keeps no standard error of an agent, so the daemon writes the file itself.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Set to [`TO_FILE`] by the `LaunchAgent`; unset, the log goes to standard error.
pub const LOG_VARIABLE: &str = "ANIMA_DAEMON_LOG";

pub const TO_FILE: &str = "file";

/// With the one older file, at most twice this stays on disk.
pub const BOUND: u64 = 1024 * 1024;

const FOLDER: &str = "Library/Logs/Anima";
const NAME: &str = "anima-daemon.log";

#[must_use]
pub fn path(env: impl Fn(&str) -> Option<String>, macos: bool) -> Option<PathBuf> {
    if !macos || env(LOG_VARIABLE).as_deref() != Some(TO_FILE) {
        return None;
    }
    let home = env("HOME").filter(|home| Path::new(home).is_absolute())?;
    Some(Path::new(&home).join(FOLDER).join(NAME))
}

#[must_use]
pub fn older(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

#[derive(Debug)]
pub struct LogFile {
    path: PathBuf,
    bound: u64,
    file: File,
    size: u64,
}

impl LogFile {
    /// # Errors
    /// The I/O error when the folder or the file cannot be made or opened.
    pub fn open(path: PathBuf, bound: u64) -> io::Result<Self> {
        if let Some(folder) = path.parent() {
            make_folder(folder)?;
        }
        let file = append(&path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path,
            bound,
            file,
            size,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        fs::rename(&self.path, older(&self.path))?;
        self.file = append(&self.path)?;
        self.size = 0;
        Ok(())
    }
}

impl Write for LogFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let incoming = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if self.size > 0 && self.size.saturating_add(incoming) > self.bound {
            self.rotate()?;
        }
        let written = self.file.write(buf)?;
        self.size = self
            .size
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

fn make_folder(folder: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn met_019_the_launch_agent_log_is_in_the_logs_folder_of_the_user() {
        // `/Users/me` is relative on Windows, so the home is under the temporary folder.
        let home = std::env::temp_dir().join("me");
        let home = home.to_str().expect("a home in text");
        let pairs = [(LOG_VARIABLE, TO_FILE), ("HOME", home)];
        let set = env(&pairs);
        assert_eq!(
            path(&set, true),
            Some(PathBuf::from(home).join("Library/Logs/Anima/anima-daemon.log"))
        );
        assert_eq!(path(&set, false), None, "only macOS writes the file");
        assert_eq!(path(env(&[("HOME", "/Users/me")]), true), None);
        assert_eq!(
            path(env(&[(LOG_VARIABLE, "other"), ("HOME", "/Users/me")]), true),
            None
        );
        assert_eq!(path(env(&[(LOG_VARIABLE, TO_FILE)]), true), None);
        assert_eq!(
            path(env(&[(LOG_VARIABLE, TO_FILE), ("HOME", "me")]), true),
            None
        );
    }

    #[test]
    fn met_019_the_file_rotates_at_its_bound_and_keeps_one_older_file() {
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join("Logs/Anima/anima-daemon.log");
        let mut log = LogFile::open(file.clone(), 30).expect("opens");
        for line in ["first line 1234\n", "second line\n"] {
            log.write_all(line.as_bytes()).expect("writes");
        }
        assert_eq!(
            fs::read_to_string(&file).expect("reads"),
            "first line 1234\nsecond line\n",
            "28 bytes stay under the bound of 30"
        );
        for line in ["third line\n", "fourth line 123456\n", "fifth\n"] {
            log.write_all(line.as_bytes()).expect("writes");
        }
        assert_eq!(fs::read_to_string(&file).expect("reads"), "fifth\n");
        assert_eq!(
            fs::read_to_string(older(&file)).expect("reads"),
            "third line\nfourth line 123456\n",
            "the first file went at the second rotation"
        );
        assert!(!folder.path().join("Logs/Anima/anima-daemon.log.2").exists());
    }

    #[test]
    fn met_019_a_line_longer_than_the_bound_is_still_written_whole() {
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join("anima-daemon.log");
        let mut log = LogFile::open(file.clone(), 4).expect("opens");
        log.write_all(b"a long line\n").expect("writes");
        assert_eq!(fs::read_to_string(&file).expect("reads"), "a long line\n");
    }

    #[test]
    fn met_019_an_open_counts_what_the_file_already_holds() {
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join("anima-daemon.log");
        fs::write(&file, "0123456789\n").expect("writes");
        let mut log = LogFile::open(file.clone(), 16).expect("opens");
        log.write_all(b"next line\n").expect("writes");
        assert_eq!(fs::read_to_string(&file).expect("reads"), "next line\n");
        assert_eq!(
            fs::read_to_string(older(&file)).expect("reads"),
            "0123456789\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn log_007_only_the_user_can_read_the_file_and_its_folder() {
        use std::os::unix::fs::PermissionsExt as _;
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join("Anima/anima-daemon.log");
        LogFile::open(file.clone(), BOUND).expect("opens");
        let mode = |path: &Path| fs::metadata(path).expect("exists").permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&folder.path().join("Anima")), 0o700);
    }
}
