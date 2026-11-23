//! BLD-040 to BLD-047. Every command runs under the step's claim, so one step runs at a time and a
//! cancel stops it.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use super::{
    artifact,
    carried::{link, mach_o::TeamId, rust_programs, signer},
    child::Claim,
    environment::Environment,
    error::BuilderError,
    job::{Commit, Target, Version},
    plan::{self, CLONE_CONFIG_KEYS, Command, Program, Repository},
};

pub struct Workshop<'a> {
    pub claim: Claim<'a>,
    pub env: Environment,
    pub repository: Repository,
    pub clone: PathBuf,
}

/// Files that make git read objects or history from elsewhere, or hide a change from `git status`.
/// The checkout removes `info/exclude`, which `git clone` writes; a build may write any (D-15,
/// D-21).
const UNTRUSTED_FILES: &[&str] = &[
    "info/grafts",
    "info/exclude",
    "info/attributes",
    "info/sparse-checkout",
    "shallow",
    "commondir",
    "objects/info/alternates",
    "objects/info/http-alternates",
    "objects/info/commit-graph",
    "objects/info/commit-graphs",
];

/// A build can mark the index to hide a changed file, and exclude rules hide an untracked one.
/// git builds the index anew from the commit.
const FORGOTTEN_FILES: &[&str] = &["index", "info/exclude"];

pub fn config_is_clones(keys: &[String]) -> bool {
    keys.iter()
        .all(|key| CLONE_CONFIG_KEYS.contains(&key.to_ascii_lowercase().as_str()))
}

impl Workshop<'_> {
    pub(super) async fn run(&self, command: &Command) -> Result<(), BuilderError> {
        let program = self.program(command.program)?;
        self.claim
            .run(command, &program, self.variables(command))
            .await
    }

    pub(super) async fn read(&self, command: &Command) -> Result<Vec<String>, BuilderError> {
        let program = self.program(command.program)?;
        self.claim
            .read(command, &program, self.variables(command))
            .await
    }

    pub fn program(&self, program: Program) -> Result<PathBuf, BuilderError> {
        match program.fixed_path() {
            Some(path) => path.is_file().then(|| path.to_owned()),
            None => self.env.find(program.name(), Path::is_file),
        }
        .ok_or(BuilderError::ToolMissing)
    }

    /// Cargo's output folder stays inside the clone, whatever the builder's cargo configuration
    /// names (BLD-044).
    pub fn variables(&self, command: &Command) -> Vec<(OsString, OsString)> {
        let mut variables = self.env.for_step(&command.env);
        variables.push((
            "CARGO_TARGET_DIR".into(),
            artifact::target_folder(&self.clone).into(),
        ));
        variables
    }

    async fn run_all(&self, commands: &[Command]) -> Result<(), BuilderError> {
        for command in commands {
            self.run(command).await?;
        }
        Ok(())
    }

    /// No file or config key that a build could have added to redirect or rewrite history (D-15,
    /// D-21).
    async fn trusted_clone(&self) -> bool {
        let git = self.clone.join(".git");
        if !git.is_dir() || UNTRUSTED_FILES.iter().any(|file| git.join(file).exists()) {
            return false;
        }
        self.read(&plan::config_keys(&self.clone))
            .await
            .is_ok_and(|keys| config_is_clones(&keys))
    }

    /// A link there is removed as a link.
    fn forget_index(&self) -> Result<(), BuilderError> {
        for file in FORGOTTEN_FILES {
            let path = self.clone.join(".git").join(file);
            match fs::symlink_metadata(&path) {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(&path),
                Ok(_) => fs::remove_file(&path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
            .map_err(|_| BuilderError::Failed)?;
        }
        Ok(())
    }

    async fn clone_anew(&self) -> Result<(), BuilderError> {
        if self.clone.exists() {
            fs::remove_dir_all(&self.clone).map_err(|_| BuilderError::Failed)?;
        }
        let parent = self.clone.parent().ok_or(BuilderError::Failed)?;
        #[cfg(windows)]
        anima_windows::files::directory(parent).map_err(|_| BuilderError::Failed)?;
        // Mode 0700 whatever the umask (0002 on Ubuntu): the tree's work folder is made beside
        // the clone, and `renew` refuses a folder that the group can write to (D-1).
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(fs::DirBuilder::new().recursive(true), 0o700)
            .create(parent)
            .map_err(|_| BuilderError::Failed)?;
        self.run(&plan::clone(&self.repository, &self.clone))
            .await?;
        Ok(())
    }

    /// A clone that fails a check is cloned anew once; a fresh clone that fails `fsck` is refused
    /// (D-15, D-21).
    async fn whole_clone(&self) -> Result<(), BuilderError> {
        let fresh = !self.trusted_clone().await;
        if fresh {
            self.clone_anew().await?;
        }
        self.run(&plan::fetch(&self.repository, &self.clone))
            .await?;
        match self.run(&plan::fsck(&self.clone)).await {
            Err(BuilderError::StepFailed) if !fresh => {}
            other => return other,
        }
        self.clone_anew().await?;
        self.run(&plan::fetch(&self.repository, &self.clone))
            .await?;
        self.run(&plan::fsck(&self.clone)).await
    }

    /// Refuses a commit that is not on `main` (BLD-040, BLD-041).
    pub async fn checkout(&self, commit: &Commit) -> Result<(), BuilderError> {
        self.whole_clone().await?;
        match self.run(&plan::ancestry(&self.clone, commit)).await {
            Err(BuilderError::StepFailed) => return Err(BuilderError::NotOnMain),
            other => other?,
        }
        self.forget_index()?;
        self.run_all(&plan::checkout(&self.clone, commit)).await?;
        if self.read(&plan::status(&self.clone)).await?.is_empty() {
            Ok(())
        } else {
            Err(BuilderError::Dirty)
        }
    }

    /// A later step cannot build another tree than the one checked out. The index is built anew
    /// from
    /// the commit first, so no mark a build put on an entry hides a change.
    pub(super) async fn at(&self, commit: &Commit) -> Result<(), BuilderError> {
        if !self.trusted_clone().await {
            return Err(BuilderError::Dirty);
        }
        let head = self.read(&plan::head(&self.clone)).await?;
        if !matches!(head.as_slice(), [line] if line == commit.as_str()) {
            return Err(BuilderError::Dirty);
        }
        self.forget_index()?;
        self.run(&plan::read_tree(&self.clone, commit)).await?;
        if self.read(&plan::status(&self.clone)).await?.is_empty() {
            Ok(())
        } else {
            Err(BuilderError::Dirty)
        }
    }

    /// `bun run gates` at `commit` (BLD-042).
    pub async fn gates(&self, commit: &Commit) -> Result<(), BuilderError> {
        self.at(commit).await?;
        self.run(&plan::gates(&self.clone, commit)).await?;
        Ok(())
    }

    /// The updater file is made from the stapled app (BLD-084, BLD-085).
    async fn notarize(&self, dmg: &Path, app: &Path) -> Result<(), BuilderError> {
        if !artifact::notarized(&self.read(&plan::notarize(dmg)).await?) {
            return Err(BuilderError::StepFailed);
        }
        self.run_all(&plan::staple(dmg)).await?;
        self.run_all(&plan::staple(app)).await
    }

    /// INS-002, BLD-043, BLD-047, BLD-083 to BLD-085, S12 2.2. `cargo build` runs code from the
    /// commit,
    /// so the committed `tauri.conf.json` is checked again after it, before Tauri.
    pub async fn bundle(
        &self,
        commit: &Commit,
        target: Target,
        version: &Version,
    ) -> Result<PathBuf, BuilderError> {
        self.at(commit).await?;
        let signer = self.signer(commit, target).await?;
        self.tree(commit, target, signer.as_ref()).await?;
        self.link(commit, target).await?;
        self.stage(target, &link::copies(&self.clone, target), &[link::PROGRAM])
            .await?;
        self.check_staged(target, &[link::PROGRAM]).await?;
        let binaries = rust_programs::PROGRAMS.map(|(_, binary)| binary);
        self.rust_programs(target, version).await?;
        self.stage(
            target,
            &rust_programs::copies(&self.clone, target),
            &binaries,
        )
        .await?;
        self.check_staged(target, &binaries).await?;
        self.committed(commit, &[signer::CONFIG]).await?;
        artifact::clear_bundle(&self.clone, target)?;
        let identity = signer.as_ref().map(|signer| signer.identity.as_str());
        self.run_all(&plan::bundle(&self.clone, target, version, identity))
            .await?;
        artifact::stabilize_names(&self.clone, target, version)?;
        let updater = artifact::updater_file(&self.clone, target, version);
        if let Some(signer) = &signer {
            let app = artifact::own_folder(&self.clone, &artifact::mac_app(&self.clone))?;
            self.check_app(&app, &signer.team).await?;
            self.release_mac(version, &app, &updater).await?;
        }
        artifact::own_file(&self.clone, &updater)
    }

    /// The checks of UPD-011 (S12 2.3).
    pub(super) async fn check_app(&self, app: &Path, team: &TeamId) -> Result<(), BuilderError> {
        let executable = artifact::mac_executable(&self.clone, app)?;
        let [codesign, lipo] = plan::bundle_checks(app, &executable);
        self.run(&codesign).await?;
        if !artifact::universal(&self.read(&lipo).await?) {
            return Err(BuilderError::Bundle);
        }
        self.signed_mach_o(app, team).await
    }

    async fn release_mac(
        &self,
        version: &Version,
        app: &Path,
        updater: &Path,
    ) -> Result<(), BuilderError> {
        let dmg = artifact::dmg_file(&self.clone, Target::DarwinUniversal, version)
            .ok_or(BuilderError::Bundle)?;
        self.notarize(&artifact::own_file(&self.clone, &dmg)?, app)
            .await?;
        self.run(&plan::app_archive(app, updater)).await
    }
}
