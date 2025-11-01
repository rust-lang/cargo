use tracing::debug;

use crate::{CargoResult, core::Workspace, util::flock::is_on_nfs_mount};

/// The strategy to use for locking during a build.
///
/// A strategy is made up of multiple locking modes for different directories.
#[derive(Debug)]
pub struct LockingStrategy {
    /// The locking mode for the artifact-dir
    artifact_dir: LockingMode,
    /// The locking mode for the build-dir
    ///
    /// Will be `None` when artifact_dir and build_dir are the same directory.
    build_dir: Option<LockingMode>,
}

impl LockingStrategy {
    /// Determines the locking strategy the current environment can support.
    pub fn determine_locking_strategy(ws: &Workspace<'_>) -> CargoResult<Self> {
        let artifact_dir_locking_mode = match is_on_nfs_mount(ws.target_dir().as_path_unlocked()) {
            true => {
                debug!("NFS detected. Disabling file system locking for artifact-dir");
                LockingMode::Disabled
            }
            false => LockingMode::Coarse,
        };
        let build_dir_locking_mode = if ws.target_dir() == ws.build_dir() {
            None
        } else {
            Some(match is_on_nfs_mount(ws.build_dir().as_path_unlocked()) {
                true => {
                    debug!("NFS detected. Disabling file system locking for build-dir");
                    LockingMode::Disabled
                }
                false => LockingMode::Coarse,
            })
        };
        Ok(Self {
            artifact_dir: artifact_dir_locking_mode,
            build_dir: build_dir_locking_mode,
        })
    }

    pub fn artifact_dir(&self) -> &LockingMode {
        &self.artifact_dir
    }

    pub fn build_dir(&self) -> &LockingMode {
        self.build_dir.as_ref().unwrap_or(&self.artifact_dir)
    }

    /// If the artifact_dir and build_dir are the same directory.
    pub fn is_unified_output_dir(&self) -> bool {
        self.build_dir.is_none()
    }
}

/// The locking mode that will be used for output directories.
#[derive(Debug)]
pub enum LockingMode {
    /// Completely disables locking (used for filesystems that do not support locking)
    Disabled,
    /// Coarse grain locking (Profile level)
    Coarse,
}
