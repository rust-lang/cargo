use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::bail;
use blake3::Hash;
use cargo_util::paths::{self, create_dir_all};
use serde::{Deserialize, Serialize};
use tracing::{instrument, trace};

use crate::CargoResult;
use crate::compiler::layout::SharedStorageLayout;

pub const VERSION: &str = "v1";

pub struct SharedStorage {
    layout: SharedStorageLayout,
    force_hardlink: bool,
}

impl SharedStorage {
    pub fn new(root: PathBuf, build_dir: &Path) -> CargoResult<Self> {
        create_dir_all(&root)?;

        if !is_same_filesystem(&root, build_dir).unwrap_or(false) {
            bail!("object storage and build-dir are on different file systems")
        }

        #[expect(
            clippy::disallowed_methods,
            reason = "testing only, no reason for config support"
        )]
        let force_hardlink =
            std::env::var_os("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK").is_some();

        Ok(Self {
            layout: SharedStorageLayout::new(&root),
            force_hardlink,
        })
    }

    pub fn insert_or_dedup_out_dir(&self, out_dir: &Path) -> CargoResult<String> {
        let mut objects = BTreeMap::new();
        for entry in walkdir::WalkDir::new(out_dir) {
            let Ok(entry) = entry else {
                continue;
            };
            if entry.file_type().is_file() {
                let hash = self
                    .insert_or_dedup_file(entry.path())?
                    .to_hex()
                    .to_string();
                trace!(path = ?entry.path(), hash, "inserted file into object storage");
                let path = entry.path().strip_prefix(out_dir)?.to_path_buf();
                objects.insert(path, hash);
            }
        }

        let buf = serde_json::to_vec(&UnitOutputEntry { objects })?;
        let unit_output_hash = blake3::hash(&buf).to_hex().to_string();
        let path = self.layout.object(&unit_output_hash);

        if !path.exists() {
            create_dir_all(&path.parent().unwrap())?;
            paths::write_atomic(path, buf)?;
        }

        Ok(unit_output_hash)
    }

    pub fn save_build_outputs(&self, hashes: &BTreeSet<String>) -> CargoResult<()> {
        let manifest = BuildManifest {
            unit_outputs: hashes.clone(),
        };

        let output = serde_json::to_vec(&manifest)?;

        let mut hasher = blake3::Hasher::new();
        hasher.update(&output);
        let hash = hasher.finalize().to_hex().to_string();

        let path = self.layout.object(&hash);

        if !path.exists() {
            create_dir_all(path.parent().unwrap())?;
            paths::write_atomic(path, output)?;
        }

        let usage = self.layout.build_index_entry(&hash);
        create_dir_all(usage.parent().unwrap())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();

        paths::write_atomic(usage, now.as_bytes())?;

        Ok(())
    }

    pub fn unit_output_exists(&self, hash: &str) -> bool {
        debug_assert_eq!(hash.len(), 64);
        return self.layout.object(hash).exists();
    }

    /// Inserts or deduplicates a file already in object storage.
    ///
    /// This function will prefer reflink if available on the current filesystem but fallback to
    /// hardlinking.
    fn insert_or_dedup_file(&self, path: &Path) -> CargoResult<Hash> {
        let hash = hash(path)?;
        let storage_path = self.layout.object(&hash.to_hex());

        if !self.insert(path, &storage_path)? {
            self.dedup(path, &storage_path)?;
        }

        Ok(hash)
    }

    fn insert(&self, path: &Path, storage_path: &Path) -> CargoResult<bool> {
        if storage_path.try_exists()? {
            return Ok(false);
        }

        create_dir_all(&storage_path.parent().unwrap())?;
        create_dir_all(&self.layout.staging)?;

        // This logic is a bit subtle.
        // Reflinking is not atomic so we create a temp dir to create that file falling back to
        // hardlinking. Then regardless of whether we reflinked or hardlinked, we hard link that
        // file into the object storage so the insert is always atomic.
        //
        // Importantly, we do not use `std::fs::rename` to move the file in to the object storage as
        // that would overwrite the existing file if there was another process inserted before us.
        let staging_dir = tempfile::tempdir_in(&self.layout.staging)?;
        let staged = staging_dir.path().join("artifact");
        if self.force_hardlink || reflink_copy::reflink(path, &staged).is_err() {
            std::fs::hard_link(path, &staged)?;
        }
        #[cfg(target_os = "linux")]
        ensure_no_writers(&staged)?;

        match std::fs::hard_link(&staged, storage_path) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn dedup(&self, path: &Path, storage_path: &Path) -> CargoResult<()> {
        let metadata = path.metadata()?;
        let staging_dir = tempfile::Builder::new()
            .prefix(".object")
            .tempdir_in(path.parent().unwrap())?;
        let staged = staging_dir.path().join("artifact");
        if !self.force_hardlink && reflink_copy::reflink(storage_path, &staged).is_ok() {
            // Reflink creates a different inode so things like mtimes are not preserved
            // automatically, which causes issues with rebuild detection.
            std::fs::set_permissions(&staged, metadata.permissions())?;
            let times = std::fs::FileTimes::new()
                .set_accessed(metadata.accessed()?)
                .set_modified(metadata.modified()?);
            std::fs::set_times(&staged, times)?;
        } else {
            std::fs::hard_link(storage_path, &staged)?;
        }
        #[cfg(target_os = "linux")]
        ensure_no_writers(&staged)?;

        // A pipelined rustc invocation may already be reading this .rmeta file.
        // Rename the staged file into place so existing readers keep reading the
        // original file. On Windows, an open handle can prevent replacement.
        // If that happens, the rename fails and the original file stays in place.
        std::fs::rename(&staged, path)?;

        Ok(())
    }
}

#[instrument]
fn hash(path: &Path) -> CargoResult<Hash> {
    let mut hasher = blake3::Hasher::new();
    let file = File::open(path)?;
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}

/// Errors if we cannot get a read lease to a file (ensuring no writers)
///
/// On Linux we need this to avoid concurrency issues when deduplicating with reflinks.
/// During reflinking, there is a brief window where we hold a write lease to the file.
/// If during this period, another worker fork's (say to spawn a rustc process) that process
/// will inhierit the writable fd. This is problematic as we cannot execute a file that has a
/// writable fd which breaks things like executing build scripts.
#[cfg(target_os = "linux")]
fn ensure_no_writers(path: &Path) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    let file = File::open(path)?;
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLEASE, libc::F_RDLCK) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLEASE, libc::F_UNLCK) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn is_same_filesystem(dir1: &Path, dir2: &Path) -> std::io::Result<bool> {
    #[expect(
        clippy::disallowed_methods,
        reason = "testing only, no reason for config support"
    )]
    if std::env::var_os("__CARGO_TEST_SHARED_STORAGE_DIFFERENT_FILESYSTEM").is_some() {
        return Ok(false);
    }

    cfg_select! {
        unix => {
            use std::os::unix::fs::MetadataExt;
            let meta1 = std::fs::metadata(dir1)?;
            let meta2 = std::fs::metadata(dir2)?;
            Ok(meta1.dev() == meta2.dev())
        }
        windows => {
            use std::fs::OpenOptions;
            use std::os::windows::fs::OpenOptionsExt;
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
                GetFileInformationByHandle,
            };

            // FIXME: Ideally we use std if/when https://github.com/rust-lang/rust/issues/63010 is
            // stabilized
            fn volume_serial_number(path: &Path) -> std::io::Result<u32> {
                let file = OpenOptions::new()
                    .access_mode(0)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                    .open(path)?;
                let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(info.dwVolumeSerialNumber)
            }

            Ok(volume_serial_number(dir1)? == volume_serial_number(dir2)?)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitOutputEntry {
    pub objects: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildManifest {
    pub unit_outputs: BTreeSet<String>,
}
