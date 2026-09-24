use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::bail;
use cargo_util::paths::create_dir_all;
use filetime::FileTime;
use tracing::{instrument, warn};

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
        let Err(err) = reflink_copy::reflink(path, storage_path) else {
            return Ok(true);
        };

        if err.kind() == ErrorKind::AlreadyExists {
            return Ok(false);
        }

        match std::fs::hard_link(path, storage_path) {
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
        if reflink_copy::reflink(storage_path, &replacement).is_err() {
            std::fs::hard_link(storage_path, &replacement)?;
        } else {
            // Reflink creates a different inode so things like mtimes are not preserved
            // automatically, which causes issues with rebuild detection.
            std::fs::set_permissions(&replacement, metadata.permissions())?;
            filetime::set_file_times(
                &replacement,
                FileTime::from_last_access_time(&metadata),
                FileTime::from_last_modification_time(&metadata),
            )?;
        }
        std::fs::rename(&replacement, path)?;

        Ok(())
    }

    #[instrument]
    pub fn hash(path: &Path) -> CargoResult<String> {
        let mut hasher = blake3::Hasher::new();
        let file = std::fs::File::open(path)?;
        hasher.update_reader(file)?;
        Ok(hasher.finalize().to_hex().to_string())
    }
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

/// A cache of file hashes to speed up file hashing when dealing with shared storage.
///
/// The idea with the hash cache, is that we need to get file hashes to insert them into
/// [`BlobStorage`] but for large workspaces the time to hash everything really adds up.
/// As an optimization we hash the file's metadata (see [`HashCache::file_metadata_hash`]) and
/// keep a mapping of the metadata hash to the file hash. It's usually much faster to hash the files
/// metadata instead of the whole file. If the file changed the metadata will update indicating that
/// we need to rehash the file to get the new checksum.
///
/// This struct is fairly specialized for a specific usecase so the behavior might seem a bit odd at
/// first. When loading a cache, we are expecting the caller to check all of the files for a build
/// unit. We want to clean up any stale files that no longer exist so we keep track of the calls
/// they make to [`HashCache::get`] and during drop if the calls did not match what was originally
/// loaded, we rewrite the file on Drop so its up to date and does not grow over time.
pub struct HashCache {
    file: PathBuf,
    entries: BTreeMap<u64, String>,
    cached_entries: Option<BTreeMap<u64, String>>,
    out_dir: PathBuf,
}

impl HashCache {
    const FILE_NAME: &'static str = ".hashes";

    pub fn new(unit_dir: &Path, out_dir: PathBuf) -> Self {
        Self {
            file: unit_dir.join(Self::FILE_NAME),
            entries: BTreeMap::new(),
            cached_entries: None,
            out_dir,
        }
    }

    pub fn load(unit_dir: &Path, out_dir: PathBuf) -> Self {
        let file = unit_dir.join(Self::FILE_NAME);
        let cached_entries = std::fs::read_to_string(&file).ok().map(|content| {
            content
                .lines()
                .filter_map(|line| {
                    let (metadata_hash, hash) = line.split_once(' ')?;
                    Some((metadata_hash.parse().ok()?, hash.to_string()))
                })
                .collect()
        });

        Self {
            file,
            entries: BTreeMap::new(),
            cached_entries,
            out_dir,
        }
    }

    pub fn get(&mut self, path: &Path) -> CargoResult<String> {
        let Ok(key) = self.file_metadata_hash(path) else {
            // If we failed to hash the metadata, just try to hash the file and don't cache it.
            return BlobStorage::hash(path);
        };

        let hash = match self.entries.entry(key) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let cached = self
                    .cached_entries
                    .as_ref()
                    .and_then(|cache| cache.get(&key));
                let hash = match cached {
                    Some(hash) => hash.clone(),
                    None => BlobStorage::hash(path)?,
                };
                entry.insert(hash)
            }
        };

        Ok(hash.clone())
    }

    pub fn insert(&mut self, path: &Path, hash: String) -> CargoResult<()> {
        self.entries.insert(self.file_metadata_hash(path)?, hash);
        Ok(())
    }

    #[instrument(skip(self))]
    fn save(&mut self) -> CargoResult<()> {
        if self.cached_entries.as_ref() == Some(&self.entries) {
            return Ok(());
        }
        let mut f = match File::create(&self.file) {
            Ok(f) => f,
            Err(err) => {
                warn!(?err, "failed to create hash cache");
                return Ok(());
            }
        };
        for (metadata_hash, file_hash) in &self.entries {
            writeln!(f, "{metadata_hash} {file_hash}")?;
        }
        f.flush()?;
        Ok(())
    }

    #[instrument(skip(self))]
    fn file_metadata_hash(&self, path: &Path) -> CargoResult<u64> {
        let metadata = path.metadata()?;
        cfg_select! {
            unix => {
                use std::os::unix::fs::MetadataExt;
                let ctime = (metadata.ctime(), metadata.ctime_nsec());
            }
            _ => {
                let ctime = metadata.created().ok();
            }
        }
        Ok(crate::util::hash_u64((
            path.strip_prefix(&self.out_dir)?,
            ctime,
            metadata.modified()?,
            metadata.len(),
        )))
    }
}

impl Drop for HashCache {
    fn drop(&mut self) {
        if let Err(err) = self.save() {
            warn!(?err, "Failed to save hash cache");
        }
    }
}
