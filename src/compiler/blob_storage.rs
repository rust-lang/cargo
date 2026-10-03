use std::fs::File;
use std::io::{ErrorKind, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::bail;
use cargo_util::paths::create_dir_all;
use filetime::FileTime;
use tracing::{debug, instrument};

use crate::CargoResult;
use crate::util::flock::{lock_exclusive, lock_shared};

const USAGE_UPDATE_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

pub struct BlobStorage {
    root: PathBuf,
}

impl BlobStorage {
    pub fn new(root: PathBuf, build_dir: &Path) -> CargoResult<Self> {
        create_dir_all(&root)?;

        if !is_same_filesystem(&root, build_dir).unwrap_or(false) {
            bail!("blob storage and build-dir are on different file systems")
        }

        Ok(Self { root })
    }

    /// Inserts or deduplicates a file already in blob storage.
    ///
    /// This function will prefer reflink if available on the current filesystem but fallback to
    /// hardlinking.
    pub fn insert_or_dedup(
        &self,
        artifact_path: &Path,
        timestamp_path: &Path,
    ) -> CargoResult<String> {
        let hash = Self::hash(artifact_path)?;
        let storage_path = self.root.join(&hash);

        if !self.insert(artifact_path, timestamp_path, &storage_path)? {
            self.dedup(artifact_path, timestamp_path, &storage_path)?;
        }

        Ok(hash)
    }

    #[instrument(skip_all)]
    pub fn mark_timestamps_used(timestamps_dir: &Path, now: SystemTime) -> CargoResult<()> {
        let now_secs = unix_timestamp(now);
        for entry in std::fs::read_dir(timestamps_dir)? {
            let Ok(entry) = entry else {
                continue;
            };
            let update = || -> CargoResult<()> {
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    return Self::mark_timestamps_used(&entry.path(), now);
                }
                if !file_type.is_file() {
                    return Ok(());
                }
                let path = entry.path();
                if is_timestamp_stale(&path, now_secs) {
                    write_used_timestamp(&path, now_secs)?;
                }
                Ok(())
            };
            if let Err(err) = update() {
                debug!(path = ?entry.path(), ?err, "failed to update blob usage");
            }
        }
        Ok(())
    }

    fn insert(
        &self,
        artifact_path: &Path,
        timestamp_path: &Path,
        storage_path: &Path,
    ) -> CargoResult<bool> {
        if storage_path.try_exists()? {
            return Ok(false);
        }

        // This logic is a bit subtle.
        // Reflinking is not atomic so we create a temp dir to create that file falling back to
        // hardlinking. Then regardless of whether we reflinked or hardlinked, we hard link that
        // file into the blob storage so the insert is always atomic.
        //
        // Importantly, we do not use `std::fs::rename` to move the file in to the blob storage as
        // that would overwrite the existing file if there was another process inserted before us.
        let staging_dir = tempfile::Builder::new()
            .prefix(".blob")
            .tempdir_in(&self.root)?;
        let staged = staging_dir.path().join("artifact");
        if reflink_copy::reflink(artifact_path, &staged).is_err() {
            std::fs::hard_link(artifact_path, &staged)?;
        }
        #[cfg(target_os = "linux")]
        ensure_no_writers(&staged)?;

        match std::fs::hard_link(&staged, storage_path) {
            Ok(()) => {
                Self::create_timestamp_file(timestamp_path, storage_path)?;

                Ok(true)
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn dedup(&self, path: &Path, timestamp_path: &Path, storage_path: &Path) -> CargoResult<()> {
        let metadata = path.metadata()?;
        let staging_dir = tempfile::Builder::new()
            .prefix(".blob")
            .tempdir_in(path.parent().unwrap())?;
        let replacement = staging_dir.path().join("artifact");
        if reflink_copy::reflink(storage_path, &replacement).is_ok() {
            // Reflink creates a different inode so things like mtimes are not preserved
            // automatically, which causes issues with rebuild detection.
            std::fs::set_permissions(&replacement, metadata.permissions())?;
            filetime::set_file_times(
                &replacement,
                FileTime::from_last_access_time(&metadata),
                FileTime::from_last_modification_time(&metadata),
            )?;
        } else {
            std::fs::hard_link(storage_path, &replacement)?;
        }
        #[cfg(target_os = "linux")]
        ensure_no_writers(&replacement)?;
        std::fs::rename(&replacement, path)?;

        Self::create_timestamp_file(timestamp_path, storage_path)?;

        Ok(())
    }

    fn create_timestamp_file(timestamp_path: &Path, storage_path: &Path) -> CargoResult<()> {
        let timestamp = storage_path.with_added_extension("timestamp");
        write_used_timestamp(&timestamp, now())?;
        create_dir_all(timestamp_path.parent().unwrap())?;
        std::fs::hard_link(&timestamp, &timestamp_path)?;
        Ok(())
    }

    #[instrument]
    fn hash(path: &Path) -> CargoResult<String> {
        let mut hasher = blake3::Hasher::new();
        let file = File::open(path)?;
        hasher.update_reader(file)?;
        Ok(hasher.finalize().to_hex().to_string())
    }
}

fn is_timestamp_stale(path: &Path, now_secs: u64) -> bool {
    match read_timestamp_file(path) {
        Some(stored) => now_secs.saturating_sub(stored) >= USAGE_UPDATE_INTERVAL.as_secs(),
        None => true,
    }
}

pub fn read_timestamp_file(path: &Path) -> Option<u64> {
    let mut contents = String::new();
    let file = File::open(path).ok()?;
    lock_shared(&file).ok()?;
    file.take(32).read_to_string(&mut contents).ok()?;
    contents.trim().parse().ok()
}

/// Writes unix seconds to a timestamp file, creating it if needed.
/// This truncates and rewrites so the hardlink is preserved.
fn write_used_timestamp(path: &Path, now_secs: u64) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    lock_exclusive(&file)?;
    write!(file, "{now_secs}")?;
    Ok(())
}

fn now() -> u64 {
    unix_timestamp(SystemTime::now())
}

fn unix_timestamp(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Errors if there we cannot get a read lease to a file (ensuring no writers)
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
