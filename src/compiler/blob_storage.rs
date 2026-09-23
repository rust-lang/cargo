use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::bail;
use cargo_util::paths::create_dir_all;
use filetime::FileTime;
use tracing::instrument;

use crate::CargoResult;

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
    pub fn insert_or_dedup(&self, path: &Path) -> CargoResult<String> {
        let hash = Self::hash(path)?;
        let storage_path = self.root.join(&hash);

        if self.insert(path, &storage_path)? {
            return Ok(hash);
        }

        self.dedup(path, &storage_path)?;

        Ok(hash)
    }

    fn insert(&self, path: &Path, storage_path: &Path) -> CargoResult<bool> {
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
        if reflink_copy::reflink(path, &staged).is_err() {
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
