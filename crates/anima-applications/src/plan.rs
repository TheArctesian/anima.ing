//! What the program would do for one item, as a value (contract K10 Sections 6.4.1 and 6.4.5).
mod version;

pub use version::Version;

/// The real paths that the program resolved at its start (Section 6.4.1). Each is absolute,
/// with each link resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub docker: String,
    pub compose: String,
    pub socket: String,
    /// The configuration folder of the program for Docker, which only the user that the
    /// program runs as can write.
    pub configuration: String,
    /// The folder of the applications that Anima installed, which only the user that the
    /// program runs as can reach (Section 6.4.5).
    pub root: String,
}

/// One program to start, directly and never through a shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// The real path of the program.
    pub program: String,
    /// The arguments, one text each. No text is read a second time by a shell.
    pub arguments: Vec<String>,
    /// The whole environment of the program: it gets no other variable.
    pub environment: Vec<(String, String)>,
    pub folder: Option<String>,
}

/// One step of a plan. The caller does the steps in their order. A step that fails stops the
/// plan: after it, the caller does only each [`Step::DeleteFolderInEachCase`]. A failed
/// [`Step::Attempt`] does not stop the plan.
///
/// The content of [`Step::Write`] can hold values of the environment file: `{:?}` shows only
/// its length.
#[derive(Clone, PartialEq, Eq)]
pub enum Step {
    /// Makes a folder that only the user of the program can reach, when it is not there.
    Folder { path: String },
    /// Writes a file that only the user of the program can read, in place of an earlier one.
    Write { path: String, content: Vec<u8> },
    /// Deletes a file, when it is there.
    Delete { path: String },
    /// Deletes a folder with all below it, when it is there, without following a link.
    DeleteFolder { path: String },
    /// Deletes a folder as [`Step::DeleteFolder`] does. It runs also when an earlier step
    /// failed and stopped the plan.
    DeleteFolderInEachCase { path: String },
    /// Stops the plan when the path exists.
    Absent { path: String },
    /// Stops the plan unless `path` is a real folder directly below the real folder
    /// `below`, checked without following a link (SF-016).
    RealFolder { path: String, below: String },
    /// Ensure a bind folder exists below the private application boundary.
    BindFolder { path: String },
    /// Starts a program and waits for its end.
    Run(Command),
    /// Start with the proposed final form and record the private version on confirmed success.
    ApplyVersion {
        command: Command,
        version: Box<Version>,
    },
    /// Starts a program and waits for its end; a failure is reported and the plan goes on.
    Attempt(Command),
}

fn one_path(formatter: &mut std::fmt::Formatter<'_>, name: &str, path: &str) -> std::fmt::Result {
    formatter.debug_struct(name).field("path", &path).finish()
}

impl std::fmt::Debug for Step {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Folder { path } => one_path(formatter, "Folder", path),
            Self::Write { path, content } => formatter
                .debug_struct("Write")
                .field("path", path)
                .field("content_bytes", &content.len())
                .finish(),
            Self::Delete { path } => one_path(formatter, "Delete", path),
            Self::DeleteFolder { path } => one_path(formatter, "DeleteFolder", path),
            Self::DeleteFolderInEachCase { path } => {
                one_path(formatter, "DeleteFolderInEachCase", path)
            }
            Self::Absent { path } => one_path(formatter, "Absent", path),
            Self::RealFolder { path, below } => formatter
                .debug_struct("RealFolder")
                .field("path", path)
                .field("below", below)
                .finish(),
            Self::Run(command) => formatter.debug_tuple("Run").field(command).finish(),
            Self::BindFolder { path } => one_path(formatter, "BindFolder", path),
            Self::ApplyVersion { command, version } => formatter
                .debug_struct("ApplyVersion")
                .field("command", command)
                .field("version", version)
                .finish(),
            Self::Attempt(command) => formatter.debug_tuple("Attempt").field(command).finish(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
}

pub(crate) struct Project<'a> {
    pub name: &'a str,
    /// The working folder of Compose, which is also the folder of the project.
    pub folder: &'a str,
    pub files: &'a [String],
}

impl Paths {
    /// Only the socket and client configuration selected by the local owner.
    pub(crate) fn environment(&self) -> Vec<(String, String)> {
        vec![
            ("DOCKER_HOST".to_owned(), format!("unix://{}", self.socket)),
            ("DOCKER_CONFIG".to_owned(), self.configuration.clone()),
        ]
    }

    fn first_arguments(&self) -> Vec<String> {
        vec![
            "--host".to_owned(),
            format!("unix://{}", self.socket),
            "--config".to_owned(),
            self.configuration.clone(),
        ]
    }

    /// `operands` come after `--`, so that none is read as an option.
    pub(crate) fn docker(&self, verbs: &[&str], operands: &[&str]) -> Command {
        let mut arguments = self.first_arguments();
        arguments.extend(verbs.iter().map(|verb| (*verb).to_owned()));
        arguments.push("--".to_owned());
        arguments.extend(operands.iter().map(|operand| (*operand).to_owned()));
        Command {
            program: self.docker.clone(),
            arguments,
            environment: self.environment(),
            folder: None,
        }
    }

    pub(crate) fn compose(&self, project: &Project<'_>, verbs: &[&str]) -> Command {
        let mut arguments = vec![
            "--project-name".to_owned(),
            project.name.to_owned(),
            "--project-directory".to_owned(),
            project.folder.to_owned(),
        ];
        for file in project.files {
            arguments.extend(["--file".to_owned(), file.clone()]);
        }
        arguments.extend(verbs.iter().map(|verb| (*verb).to_owned()));
        Command {
            program: self.compose.clone(),
            arguments,
            environment: self.environment(),
            folder: Some(project.folder.to_owned()),
        }
    }

    /// The folder of one application: `<root>/<UUID>`.
    pub(crate) fn application(&self, application: &str) -> Folders {
        Folders::at(&self.root, application)
    }
}

/// The folder of one application and the names of what it holds (Section 6.4.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    application: String,
}

impl Folders {
    pub(crate) fn at(root: &str, application: &str) -> Self {
        Self {
            application: format!("{root}/{application}"),
        }
    }

    #[must_use]
    pub fn application(&self) -> &str {
        &self.application
    }

    /// `files/`: only the user of the program can read it, and no mount can name it.
    #[must_use]
    pub fn files(&self) -> String {
        format!("{}/files", self.application)
    }

    /// `binds/`: one folder for each bind mount.
    #[must_use]
    pub fn binds(&self) -> String {
        format!("{}/binds", self.application)
    }

    pub(crate) fn file(&self, name: &str) -> String {
        format!("{}/files/{name}", self.application)
    }

    #[must_use]
    pub fn compose(&self) -> String {
        self.file("compose-as-it-came.yaml")
    }

    /// The environment file. Its name is not one that Compose reads.
    #[must_use]
    pub fn environment(&self) -> String {
        self.file("environment-of-the-application")
    }

    /// The final form, the one file that Compose reads.
    #[must_use]
    pub fn final_form(&self) -> String {
        self.file("final-form.json")
    }

    /// The image digests of the version that runs.
    #[must_use]
    pub fn digests(&self) -> String {
        self.file("digests.json")
    }

    /// Each image that the program pulled for the application.
    #[must_use]
    pub fn pulled(&self) -> String {
        self.file("pulled.json")
    }

    /// The records of the current version.
    #[must_use]
    pub fn current(&self) -> [String; 3] {
        [self.compose(), self.environment(), self.digests()]
    }

    /// The same three files of a new version; they replace [`Self::current`] only after its start did not fail.
    #[must_use]
    pub fn staged(&self) -> [String; 3] {
        [
            self.file("new-compose-as-it-came.yaml"),
            self.file("new-environment-of-the-application"),
            self.file("new-digests.json"),
        ]
    }

    #[must_use]
    pub fn kept(&self) -> [String; 3] {
        [
            self.file("kept-compose-as-it-came.yaml"),
            self.file("kept-environment-of-the-application"),
            self.file("kept-digests.json"),
        ]
    }

    /// The file that Compose would read by itself in its working folder. `files/` never
    /// holds it (Section 6.4.1).
    #[must_use]
    pub fn dot_env(&self) -> String {
        self.file(".env")
    }
}
