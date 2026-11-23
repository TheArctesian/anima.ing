//! The recipes run before `cargo build`, whose build scripts could rewrite them, and each file they use is compared with the commit first (S12 2.2, 2.3).

use std::path::{Path, PathBuf};

use super::{
    committed, joined, link,
    mach_o::{self, ARCHS, Readings, TeamId},
    on_disk, rust_programs,
    signer::{self, Signer},
    staged, tree,
};
use crate::builder::{
    artifact,
    error::BuilderError,
    job::{Commit, Target, Version},
    steps::Workshop,
    tools::readings,
};

impl Workshop<'_> {
    /// The identity this builder was built with, only when the committed `tauri.conf.json` of the clone names the same (BLD-083).
    pub async fn signer(
        &self,
        commit: &Commit,
        target: Target,
    ) -> Result<Option<Signer>, BuilderError> {
        match target {
            Target::LinuxX86_64 | Target::WindowsX86_64 => Ok(None),
            Target::DarwinUniversal => {
                self.committed(commit, &[signer::CONFIG]).await?;
                let clone = signer::in_clone(&self.clone)?;
                Signer::pinned(readings::signing_identity().as_deref(), &clone).map(Some)
            }
        }
    }

    pub async fn committed(&self, commit: &Commit, paths: &[&str]) -> Result<(), BuilderError> {
        for path in paths {
            let kind = std::fs::symlink_metadata(self.clone.join(path))
                .map_err(|_| BuilderError::Dirty)?;
            let on_disk = self.read(&committed::on_disk(&self.clone, path)).await?;
            let in_commit = self
                .read(&committed::in_commit(&self.clone, commit, path))
                .await?;
            if !kind.is_file() || !committed::same_blob(&on_disk, &in_commit) {
                return Err(BuilderError::Dirty);
            }
        }
        Ok(())
    }

    pub async fn tree(
        &self,
        commit: &Commit,
        target: Target,
        signer: Option<&Signer>,
    ) -> Result<(), BuilderError> {
        let files = if target == Target::WindowsX86_64 {
            &tree::WINDOWS_COMMITTED[..]
        } else {
            &tree::COMMITTED[..]
        };
        self.committed(commit, files).await?;
        let work = tree::work_folder(&self.clone)?;
        on_disk::renew(&work)?;
        let identity = signer.map(|signer| signer.identity.as_str());
        self.run(&tree::build(&self.clone, target, &work, identity)?)
            .await
    }

    pub async fn link(&self, commit: &Commit, target: Target) -> Result<(), BuilderError> {
        let files = if target == Target::WindowsX86_64 {
            &link::WINDOWS_COMMITTED
        } else {
            &link::COMMITTED
        };
        self.committed(commit, files).await?;
        for path in link::ABSENT {
            on_disk::absent(&self.clone.join(path))?;
        }
        self.run(&link::verify(&self.clone)).await?;
        self.run(&link::build_for(&self.clone, target)).await
    }

    pub async fn rust_programs(
        &self,
        target: Target,
        version: &Version,
    ) -> Result<(), BuilderError> {
        self.run(&rust_programs::build(&self.clone, target, version))
            .await
    }

    pub async fn stage(
        &self,
        target: Target,
        copies: &[(PathBuf, PathBuf)],
        programs: &[&str],
    ) -> Result<(), BuilderError> {
        for (built, staged) in copies {
            artifact::stage(&self.clone, built, staged)?;
        }
        for program in programs {
            if let Some(lipo) = joined(&self.clone, target, program) {
                self.run(&lipo).await?;
            }
        }
        Ok(())
    }

    pub async fn check_staged(
        &self,
        target: Target,
        programs: &[&str],
    ) -> Result<(), BuilderError> {
        for program in programs {
            let staged = staged(&self.clone, target, program);
            let carried = match target {
                Target::DarwinUniversal => {
                    artifact::universal(&self.read(&mach_o::archs(&staged)).await?)
                }
                Target::WindowsX86_64 => {
                    super::is_x86_64_pe(&artifact::header(&self.clone, &staged, 4096)?)
                }
                Target::LinuxX86_64 => {
                    super::is_x86_64_elf(&artifact::header(&self.clone, &staged, 20)?)
                }
            };
            if !carried {
                return Err(BuilderError::Bundle);
            }
        }
        Ok(())
    }

    pub async fn signed_mach_o(&self, app: &Path, team: &TeamId) -> Result<(), BuilderError> {
        for file in on_disk::mach_o_files(&self.clone, app)? {
            if !mach_o::released(&self.mach_o_readings(&file, team).await?) {
                return Err(BuilderError::Bundle);
            }
        }
        Ok(())
    }

    /// A file that is not exactly universal is not asked about either architecture: `codesign --arch` fails on a missing one.
    pub async fn mach_o_readings(
        &self,
        file: &Path,
        team: &TeamId,
    ) -> Result<Readings, BuilderError> {
        let archs = self.read(&mach_o::archs(file)).await?;
        if !artifact::universal(&archs) {
            return Ok(Readings {
                archs,
                ..Readings::default()
            });
        }
        let verified = match self.run(&mach_o::verify(file, team)).await {
            Err(BuilderError::StepFailed) => false,
            other => other.map(|()| true)?,
        };
        let mut readings = Readings {
            archs,
            verified,
            ..Readings::default()
        };
        for arch in ARCHS {
            let display = self.read(&mach_o::display(file, arch)).await?;
            readings.displays.push((arch, display));
            let entitlements = self.read(&mach_o::entitlements(file, arch)).await?;
            readings.entitlements.push((arch, entitlements));
        }
        Ok(readings)
    }
}
