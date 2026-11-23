//! Where one bundled PostgreSQL keeps its files; nothing is read or checked on disk.
//!
//! ```text
//! <root>/cluster/      the PostgreSQL data folder
//! <root>/urls/         owner.url and hub.url; the hub's ANIMA_DATABASE_URL_FILE names hub.url
//! <root>/server.log    the server's own log
//! <root>/lock          held by the one copy that runs this database
//! <socket folder>/     the Unix socket; the server has no TCP listener (DB-001)
//! ```

use std::path::{Path, PathBuf};

#[cfg(all(test, unix))]
mod tests;

/// PostgreSQL names its socket file after the port, also when it listens on no port.
const SOCKET_FILE: &str = ".s.PGSQL.5432";

/// macOS `sun_path` holds 104 bytes with the final NUL; Linux holds more, so one limit suits both.
const SOCKET_PATH_LIMIT: usize = 103;

const MACOS_APPLICATION: &str = "ing.anima.desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Linux,
    #[cfg(windows)]
    Windows,
}

impl Platform {
    #[cfg(windows)]
    pub const HOST: Self = Self::Windows;
    #[cfg(not(windows))]
    pub const HOST: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else {
        Self::Linux
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserFolders {
    pub home: PathBuf,
    #[cfg(windows)]
    pub local_app_data: Option<PathBuf>,
    /// `XDG_DATA_HOME`. Linux only.
    pub xdg_data_home: Option<PathBuf>,
    /// `XDG_RUNTIME_DIR`. Linux only.
    pub xdg_runtime_dir: Option<PathBuf>,
}

/// The two logins that get a URL file (R-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Applies migrations and owns every object.
    Owner,
    /// The hub's runtime login, which owns nothing.
    Hub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("the {folder} is not an absolute path")]
    NotAbsolute { folder: &'static str },
    #[error("the {folder}'s path has a control character")]
    ControlCharacter { folder: &'static str },
    #[error(
        "the database socket path is {length} bytes and the limit is {limit}; \
         the home folder path is too long"
    )]
    SocketPathTooLong { length: usize, limit: usize },
    #[error(
        "the socket folder's path has a comma, a quotation mark or a space at an end, or is \
         not UTF-8; PostgreSQL would read another folder from it"
    )]
    SocketFolderList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
    socket_folder: PathBuf,
}

impl Layout {
    /// A layout with its files below `root` and its socket in `socket_folder`.
    ///
    /// # Errors
    /// When a folder is not absolute, or the socket file's path is too long for a Unix socket.
    pub fn new(root: PathBuf, socket_folder: PathBuf) -> Result<Self, LayoutError> {
        absolute(&root, "database root")?;
        absolute(&socket_folder, "socket folder")?;
        // PostgreSQL writes both paths as lines of `postmaster.pid`; a line break would shift them.
        for (folder, name) in [(&root, "database root"), (&socket_folder, "socket folder")] {
            let has_control = folder
                .as_os_str()
                .as_encoded_bytes()
                .iter()
                .any(u8::is_ascii_control);
            if has_control {
                return Err(LayoutError::ControlCharacter { folder: name });
            }
        }
        // `unix_socket_directories` is a comma list whose entries PostgreSQL trims.
        let listed = socket_folder
            .to_str()
            .is_none_or(|text| text.contains([',', '"']) || text.trim() != text);
        if listed {
            return Err(LayoutError::SocketFolderList);
        }
        let layout = Self {
            root,
            socket_folder,
        };
        let length = layout.socket_file().as_os_str().as_encoded_bytes().len();
        if cfg!(unix) && length > SOCKET_PATH_LIMIT {
            return Err(LayoutError::SocketPathTooLong {
                length,
                limit: SOCKET_PATH_LIMIT,
            });
        }
        Ok(layout)
    }

    /// The layout the application uses for `user` on `platform`.
    ///
    /// macOS keeps the socket in a short folder: `Application Support` takes much of the path limit.
    ///
    /// # Errors
    /// As [`Layout::new`]; the home folder is the path that can be too long.
    pub fn for_user(platform: Platform, user: &UserFolders) -> Result<Self, LayoutError> {
        absolute(&user.home, "home folder")?;
        match platform {
            #[cfg(windows)]
            Platform::Windows => {
                let application = given(user.local_app_data.as_deref())
                    .map_or_else(|| user.home.join("AppData/Local"), Path::to_path_buf)
                    .join("Anima");
                Self::new(application.join("hub/database"), application.join("hub/pg"))
            }
            Platform::MacOs => {
                let application = user
                    .home
                    .join("Library/Application Support")
                    .join(MACOS_APPLICATION);
                Self::new(application.join("hub/database"), application.join("pg"))
            }
            Platform::Linux => {
                let share = given(user.xdg_data_home.as_deref())
                    .map_or_else(|| user.home.join(".local/share"), Path::to_path_buf)
                    .join("anima");
                let socket_folder = given(user.xdg_runtime_dir.as_deref())
                    .map_or_else(|| share.join("pg"), |runtime| runtime.join("anima"));
                Self::new(share.join("hub/database"), socket_folder)
            }
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The PostgreSQL data folder. PostgreSQL's word for it is "cluster".
    #[must_use]
    pub fn cluster(&self) -> PathBuf {
        self.root.join("cluster")
    }

    /// Where a new cluster is made before it takes its place; one that a crash left is removed.
    #[must_use]
    pub fn cluster_being_made(&self) -> PathBuf {
        self.root.join("cluster.new")
    }

    /// The folder of the URL files. Each file in it holds a password.
    #[must_use]
    pub fn urls_folder(&self) -> PathBuf {
        self.root.join("urls")
    }

    #[must_use]
    pub fn url_file(&self, role: Role) -> PathBuf {
        self.urls_folder().join(match role {
            Role::Owner => "owner.url",
            Role::Hub => "hub.url",
        })
    }

    #[must_use]
    pub fn server_log(&self) -> PathBuf {
        self.root.join("server.log")
    }

    #[must_use]
    pub fn lock_file(&self) -> PathBuf {
        self.root.join("lock")
    }

    #[must_use]
    pub fn socket_folder(&self) -> &Path {
        &self.socket_folder
    }

    #[must_use]
    pub fn socket_file(&self) -> PathBuf {
        self.socket_folder.join(SOCKET_FILE)
    }
}

fn absolute(path: &Path, folder: &'static str) -> Result<(), LayoutError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(LayoutError::NotAbsolute { folder })
    }
}

/// An XDG folder counts only when absolute, as the XDG Base Directory specification says.
fn given(folder: Option<&Path>) -> Option<&Path> {
    folder.filter(|path| path.is_absolute())
}
