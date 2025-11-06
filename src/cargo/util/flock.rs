//! File-locking support.
//!
//! This module defines the [`Filesystem`] type which is an abstraction over a
//! filesystem, ensuring that access to the filesystem is only done through
//! coordinated locks.
//!
//! The [`FileLock`] type represents a locked file, and provides access to the
//! file.

use std::fs::TryLockError;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Display, Path, PathBuf};

use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::style;
use anyhow::Context as _;
use cargo_util::paths;

/// A locked file.
///
/// This provides access to file while holding a lock on the file. This type
/// implements the [`Read`], [`Write`], and [`Seek`] traits to provide access
/// to the underlying file.
///
/// Locks are either shared (multiple processes can access the file) or
/// exclusive (only one process can access the file).
///
/// This type is created via methods on the [`Filesystem`] type.
///
/// When this value is dropped, the lock will be released.
#[derive(Debug)]
pub struct FileLock {
    f: Option<File>,
    path: PathBuf,
}

impl FileLock {
    /// Returns the underlying file handle of this lock.
    pub fn file(&self) -> &File {
        self.f.as_ref().unwrap()
    }

    /// Returns the underlying path that this lock points to.
    ///
    /// Note that special care must be taken to ensure that the path is not
    /// referenced outside the lifetime of this lock.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the parent path containing this file
    pub fn parent(&self) -> &Path {
        self.path.parent().unwrap()
    }

    /// Removes all sibling files to this locked file.
    ///
    /// This can be useful if a directory is locked with a sentinel file but it
    /// needs to be cleared out as it may be corrupt.
    pub fn remove_siblings(&self) -> CargoResult<()> {
        let path = self.path();
        for entry in path.parent().unwrap().read_dir()? {
            let entry = entry?;
            if Some(&entry.file_name()[..]) == path.file_name() {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                paths::remove_dir_all(entry.path())?;
            } else {
                paths::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    /// Renames the file and updates the internal path.
    ///
    /// This method performs a filesystem rename operation using [`std::fs::rename`]
    /// while keeping the FileLock's internal path synchronized with the actual
    /// file location.
    ///
    /// ## Difference from `std::fs::rename`
    ///
    /// - `std::fs::rename(old, new)` only moves the file on the filesystem
    /// - `FileLock::rename(new)` moves the file AND updates `self.path` to point to the new location
    pub fn rename<P: AsRef<Path>>(&mut self, new_path: P) -> CargoResult<()> {
        let new_path = new_path.as_ref();
        std::fs::rename(&self.path, new_path).with_context(|| {
            format!(
                "failed to rename {} to {}",
                self.path.display(),
                new_path.display()
            )
        })?;
        self.path = new_path.to_path_buf();
        Ok(())
    }
}

impl Read for FileLock {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file().read(buf)
    }
}

impl Seek for FileLock {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.file().seek(to)
    }
}

impl Write for FileLock {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file().flush()
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            if let Err(e) = f.unlock() {
                tracing::warn!("failed to release lock: {e:?}");
            }
        }
    }
}

/// A "filesystem" is intended to be a globally shared, hence locked, resource
/// in Cargo.
///
/// The `Path` of a filesystem cannot be learned unless it's done in a locked
/// fashion, and otherwise functions on this structure are prepared to handle
/// concurrent invocations across multiple instances of Cargo.
///
/// The methods on `Filesystem` that open files return a [`FileLock`] which
/// holds the lock, and that type provides methods for accessing the
/// underlying file.
///
/// If the blocking methods (like [`Filesystem::open_ro_shared`]) detect that
/// they will block, then they will display a message to the user letting them
/// know it is blocked. There are non-blocking variants starting with the
/// `try_` prefix like [`Filesystem::try_open_ro_shared_create`].
///
/// The behavior of locks acquired by the `Filesystem` depend on the operating
/// system. On unix-like system, they are advisory using [`flock`], and thus
/// not enforced against processes which do not try to acquire the lock. On
/// Windows, they are mandatory using [`LockFileEx`], enforced against all
/// processes.
///
/// This **does not** guarantee that a lock is acquired. In some cases, for
/// example on filesystems that don't support locking, it will return a
/// [`FileLock`] even though the filesystem lock was not acquired. This is
/// intended to provide a graceful fallback instead of refusing to work.
/// Usually there aren't multiple processes accessing the same resource. In
/// that case, it is the user's responsibility to not run concurrent
/// processes.
///
/// [`flock`]: https://linux.die.net/man/2/flock
/// [`LockFileEx`]: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filesystem {
    root: PathBuf,
}

impl Filesystem {
    /// Creates a new filesystem to be rooted at the given path.
    pub fn new(path: PathBuf) -> Filesystem {
        Filesystem { root: path }
    }

    /// Like `Path::join`, creates a new filesystem rooted at this filesystem
    /// joined with the given path.
    pub fn join<T: AsRef<Path>>(&self, other: T) -> Filesystem {
        Filesystem::new(self.root.join(other))
    }

    /// Like `Path::push`, pushes a new path component onto this filesystem.
    pub fn push<T: AsRef<Path>>(&mut self, other: T) {
        self.root.push(other);
    }

    /// Consumes this filesystem and returns the underlying `PathBuf`.
    ///
    /// Note that this is a relatively dangerous operation and should be used
    /// with great caution!.
    pub fn into_path_unlocked(self) -> PathBuf {
        self.root
    }

    /// Returns the underlying `Path`.
    ///
    /// Note that this is a relatively dangerous operation and should be used
    /// with great caution!.
    pub fn as_path_unlocked(&self) -> &Path {
        &self.root
    }

    /// Creates the directory pointed to by this filesystem.
    ///
    /// Handles errors where other Cargo processes are also attempting to
    /// concurrently create this directory.
    pub fn create_dir(&self) -> CargoResult<()> {
        paths::create_dir_all(&self.root)
    }

    /// Returns an adaptor that can be used to print the path of this
    /// filesystem.
    pub fn display(&self) -> Display<'_> {
        self.root.display()
    }

    /// Opens read-write exclusive access to a file, returning the locked
    /// version of a file.
    ///
    /// This function will create a file at `path` if it doesn't already exist
    /// (including intermediate directories), and then it will acquire an
    /// exclusive lock on `path`. If the process must block waiting for the
    /// lock, the `msg` is printed to [`GlobalContext`].
    ///
    /// The returned file can be accessed to look at the path and also has
    /// read/write access to the underlying file.
    pub fn open_rw_exclusive_create<P>(
        &self,
        path: P,
        gctx: &GlobalContext,
        msg: &str,
    ) -> CargoResult<FileLock>
    where
        P: AsRef<Path>,
    {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        let (path, f) = self.open(path.as_ref(), &opts, true)?;
        acquire(gctx, msg, &path, &|| f.try_lock(), &|| f.lock())?;
        Ok(FileLock { f: Some(f), path })
    }

    /// A non-blocking version of [`Filesystem::open_rw_exclusive_create`].
    ///
    /// Returns `None` if the operation would block due to another process
    /// holding the lock.
    pub fn try_open_rw_exclusive_create<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> CargoResult<Option<FileLock>> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        let (path, f) = self.open(path.as_ref(), &opts, true)?;
        if try_acquire(&path, &|| f.try_lock())? {
            Ok(Some(FileLock { f: Some(f), path }))
        } else {
            Ok(None)
        }
    }

    /// Opens read-only shared access to a file, returning the locked version of a file.
    ///
    /// This function will fail if `path` doesn't already exist, but if it does
    /// then it will acquire a shared lock on `path`. If the process must block
    /// waiting for the lock, the `msg` is printed to [`GlobalContext`].
    ///
    /// The returned file can be accessed to look at the path and also has read
    /// access to the underlying file. Any writes to the file will return an
    /// error.
    pub fn open_ro_shared<P>(
        &self,
        path: P,
        gctx: &GlobalContext,
        msg: &str,
    ) -> CargoResult<FileLock>
    where
        P: AsRef<Path>,
    {
        let (path, f) = self.open(path.as_ref(), &OpenOptions::new().read(true), false)?;
        acquire(gctx, msg, &path, &|| f.try_lock_shared(), &|| {
            f.lock_shared()
        })?;
        Ok(FileLock { f: Some(f), path })
    }

    /// Opens read-only shared access to a file, returning the locked version of a file.
    ///
    /// Compared to [`Filesystem::open_ro_shared`], this will create the file
    /// (and any directories in the parent) if the file does not already
    /// exist.
    pub fn open_ro_shared_create<P: AsRef<Path>>(
        &self,
        path: P,
        gctx: &GlobalContext,
        msg: &str,
    ) -> CargoResult<FileLock> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        let (path, f) = self.open(path.as_ref(), &opts, true)?;
        acquire(gctx, msg, &path, &|| f.try_lock_shared(), &|| {
            f.lock_shared()
        })?;
        Ok(FileLock { f: Some(f), path })
    }

    /// A non-blocking version of [`Filesystem::open_ro_shared_create`].
    ///
    /// Returns `None` if the operation would block due to another process
    /// holding the lock.
    pub fn try_open_ro_shared_create<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> CargoResult<Option<FileLock>> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true);
        let (path, f) = self.open(path.as_ref(), &opts, true)?;
        if try_acquire(&path, &|| f.try_lock_shared())? {
            Ok(Some(FileLock { f: Some(f), path }))
        } else {
            Ok(None)
        }
    }

    fn open(&self, path: &Path, opts: &OpenOptions, create: bool) -> CargoResult<(PathBuf, File)> {
        let path = self.root.join(path);
        let f = opts
            .open(&path)
            .or_else(|e| {
                // If we were requested to create this file, and there was a
                // NotFound error, then that was likely due to missing
                // intermediate directories. Try creating them and try again.
                if e.kind() == io::ErrorKind::NotFound && create {
                    paths::create_dir_all(path.parent().unwrap())?;
                    Ok(opts.open(&path)?)
                } else {
                    Err(anyhow::Error::from(e))
                }
            })
            .with_context(|| format!("failed to open: {}", path.display()))?;
        Ok((path, f))
    }
}

impl PartialEq<Path> for Filesystem {
    fn eq(&self, other: &Path) -> bool {
        self.root == other
    }
}

impl PartialEq<Filesystem> for Path {
    fn eq(&self, other: &Filesystem) -> bool {
        self == other.root
    }
}

fn try_acquire(path: &Path, lock_try: &dyn Fn() -> Result<(), TryLockError>) -> CargoResult<bool> {
    // File locking on Unix is currently implemented via `flock`, which is known
    // to be broken on NFS. We could in theory just ignore errors that happen on
    // NFS, but apparently the failure mode [1] for `flock` on NFS is **blocking
    // forever**, even if the "non-blocking" flag is passed!
    //
    // As a result, we just skip all file locks entirely on NFS mounts. That
    // should avoid calling any `flock` functions at all, and it wouldn't work
    // there anyway.
    //
    // [1]: https://github.com/rust-lang/cargo/issues/2615
    if is_on_nfs_mount(path) {
        tracing::debug!("{path:?} appears to be an NFS mount, not trying to lock");
        return Ok(true);
    }

    match lock_try() {
        Ok(()) => Ok(true),

        // In addition to ignoring NFS which is commonly not working we also
        // just ignore locking on filesystems that look like they don't
        // implement file locking.
        Err(TryLockError::Error(e)) if error_unsupported(&e) => Ok(true),

        Err(TryLockError::Error(e)) => {
            let e = anyhow::Error::from(e);
            let cx = format!("failed to lock file: {}", path.display());
            Err(e.context(cx))
        }

        Err(TryLockError::WouldBlock) => Ok(false),
    }
}

/// Acquires a lock on a file in a "nice" manner.
///
/// Almost all long-running blocking actions in Cargo have a status message
/// associated with them as we're not sure how long they'll take. Whenever a
/// conflicted file lock happens, this is the case (we're not sure when the lock
/// will be released).
///
/// This function will acquire the lock on a `path`, printing out a nice message
/// to the console if we have to wait for it. It will first attempt to use `try`
/// to acquire a lock on the crate, and in the case of contention it will emit a
/// status message based on `msg` to [`GlobalContext`]'s shell, and then use `block` to
/// block waiting to acquire a lock.
///
/// Returns an error if the lock could not be acquired or if any error other
/// than a contention error happens.
fn acquire(
    gctx: &GlobalContext,
    msg: &str,
    path: &Path,
    lock_try: &dyn Fn() -> Result<(), TryLockError>,
    lock_block: &dyn Fn() -> io::Result<()>,
) -> CargoResult<()> {
    // Ensure `shell` is not already in use,
    // regardless of whether we hit contention or not
    gctx.debug_assert_shell_not_borrowed();
    if try_acquire(path, lock_try)? {
        return Ok(());
    }
    let msg = format!("waiting for file lock on {}", msg);
    gctx.shell()
        .status_with_color("Blocking", &msg, &style::NOTE)?;

    lock_block().with_context(|| format!("failed to lock file: {}", path.display()))?;
    Ok(())
}

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
pub fn is_on_nfs_mount(path: &Path) -> bool {
    use std::ffi::CString;
    use std::mem;
    use std::os::unix::prelude::*;

    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };

    unsafe {
        let mut buf: libc::statfs = mem::zeroed();
        let r = libc::statfs(path.as_ptr(), &mut buf);

        r == 0 && buf.f_type as u32 == libc::NFS_SUPER_MAGIC as u32
    }
}

#[cfg(any(not(target_os = "linux"), target_env = "musl"))]
pub fn is_on_nfs_mount(_path: &Path) -> bool {
    false
}

#[cfg(unix)]
fn error_unsupported(err: &std::io::Error) -> bool {
    match err.raw_os_error() {
        // Unfortunately, depending on the target, these may or may not be the same.
        // For targets in which they are the same, the duplicate pattern causes a warning.
        #[allow(unreachable_patterns)]
        Some(libc::ENOTSUP | libc::EOPNOTSUPP) => true,
        Some(libc::ENOSYS) => true,
        _ => err.kind() == std::io::ErrorKind::Unsupported,
    }
}

#[cfg(windows)]
fn error_unsupported(err: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::ERROR_INVALID_FUNCTION;
    match err.raw_os_error() {
        Some(code) if code == ERROR_INVALID_FUNCTION as i32 => true,
        _ => err.kind() == std::io::ErrorKind::Unsupported,
    }
}
