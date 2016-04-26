use std::fs::{self, File, OpenOptions};
use std::io::*;
use std::io;
use std::path::{Path, PathBuf, Display};

use term::color::CYAN;
use fs2::{FileExt, lock_contended_error};

use util::{CargoResult, ChainError, Config, human};

pub struct FileLock {
    f: Option<File>,
    path: PathBuf,
    state: State,
}

#[derive(PartialEq)]
enum State {
    Unlocked,
    Shared,
    Exclusive,
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
        assert!(self.state != State::Unlocked);
        &self.path
    }

    /// Returns the parent path containing this file
    pub fn parent(&self) -> &Path {
        assert!(self.state != State::Unlocked);
        self.path.parent().unwrap()
    }

    /// Removes all sibling files to this locked file.
    ///
    /// This can be useful if a directory is locked with a sentinel file but it
    /// needs to be cleared out as it may be corrupt.
    pub fn remove_siblings(&self) -> io::Result<()> {
        let path = self.path();
        for entry in try!(path.parent().unwrap().read_dir()) {
            let entry = try!(entry);
            if Some(&entry.file_name()[..]) == path.file_name() {
                continue
            }
            let kind = try!(entry.file_type());
            if kind.is_dir() {
                try!(fs::remove_dir_all(entry.path()));
            } else {
                try!(fs::remove_file(entry.path()));
            }
        }
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
        if self.state != State::Unlocked {
            if let Some(f) = self.f.take() {
                let _ = f.unlock();
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
#[derive(Clone, Debug)]
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

    /// Creates the directory pointed to by this filesystem.
    ///
    /// Handles errors where other Cargo processes are also attempting to
    /// concurrently create this directory.
    pub fn create_dir(&self) -> io::Result<()> {
        return create_dir_all(&self.root);
    }

    /// Returns an adaptor that can be used to print the path of this
    /// filesystem.
    pub fn display(&self) -> Display {
        self.root.display()
    }

    /// Opens exclusive access to a file, returning the locked version of a
    /// file.
    ///
    /// This function will create a file at `path` if it doesn't already exist
    /// (including intermediate directories), and then it will acquire an
    /// exclusive lock on `path`. If the process must block waiting for the
    /// lock, the `msg` is printed to `config`.
    ///
    /// The returned file can be accessed to look at the path and also has
    /// read/write access to the underlying file.
    pub fn open_rw<P>(&self,
                      path: P,
                      config: &Config,
                      msg: &str) -> CargoResult<FileLock>
        where P: AsRef<Path>
    {
        self.open(path.as_ref(),
                  OpenOptions::new().read(true).write(true).create(true),
                  State::Exclusive,
                  config,
                  msg)
    }

    /// Opens shared access to a file, returning the locked version of a file.
    ///
    /// This function will fail if `path` doesn't already exist, but if it does
    /// then it will acquire a shared lock on `path`. If the process must block
    /// waiting for the lock, the `msg` is printed to `config`.
    ///
    /// The returned file can be accessed to look at the path and also has read
    /// access to the underlying file. Any writes to the file will return an
    /// error.
    pub fn open_ro<P>(&self,
                      path: P,
                      config: &Config,
                      msg: &str) -> CargoResult<FileLock>
        where P: AsRef<Path>
    {
        self.open(path.as_ref(),
                  OpenOptions::new().read(true),
                  State::Shared,
                  config,
                  msg)
    }

    fn open(&self,
            path: &Path,
            opts: &OpenOptions,
            state: State,
            config: &Config,
            msg: &str) -> CargoResult<FileLock> {
        let path = self.root.join(path);

        // If we want an exclusive lock then if we fail because of NotFound it's
        // likely because an intermediate directory didn't exist, so try to
        // create the directory and then continue.
        let f = try!(opts.open(&path).or_else(|e| {
            if e.kind() == io::ErrorKind::NotFound && state == State::Exclusive {
                try!(create_dir_all(path.parent().unwrap()));
                opts.open(&path)
            } else {
                Err(e)
            }
        }).chain_error(|| {
            human(format!("failed to open: {}", path.display()))
        }));
        match state {
            State::Exclusive => {
                try!(acquire(config, msg, &path,
                             &|| f.try_lock_exclusive(),
                             &|| f.lock_exclusive()));
            }
            State::Shared => {
                try!(acquire(config, msg, &path,
                             &|| f.try_lock_shared(),
                             &|| f.lock_shared()));
            }
            State::Unlocked => {}

        }
        Ok(FileLock { f: Some(f), path: path, state: state })
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
/// status message based on `msg` to `config`'s shell, and then use `block` to
/// block waiting to acquire a lock.
///
/// Returns an error if the lock could not be acquired or if any error other
/// than a contention error happens.
fn acquire(config: &Config,
           msg: &str,
           path: &Path,
           try: &Fn() -> io::Result<()>,
           block: &Fn() -> io::Result<()>) -> CargoResult<()> {

    // File locking on Unix is currently implemented via `flock`, which is known
    // to be broken on NFS. We could in theory just ignore errors that happen on
    // NFS, but apparently the failure mode [1] for `flock` on NFS is **blocking
    // forever**, even if the nonblocking flag is passed!
    //
    // As a result, we just skip all file locks entirely on NFS mounts. That
    // should avoid calling any `flock` functions at all, and it wouldn't work
    // there anyway.
    //
    // [1]: https://github.com/rust-lang/cargo/issues/2615
    if is_on_nfs_mount(path) {
        return Ok(())
    }

    match try() {
        Ok(()) => return Ok(()),
        Err(e) => {
            if e.raw_os_error() != lock_contended_error().raw_os_error() {
                return Err(human(e)).chain_error(|| {
                    human(format!("failed to lock file: {}", path.display()))
                })
            }
        }
    }
    let msg = format!("waiting for file lock on {}", msg);
    try!(config.shell().err().say_status("Blocking", &msg, CYAN, true));

    return block().chain_error(|| {
        human(format!("failed to lock file: {}", path.display()))
    });

    #[cfg(target_os = "linux")]
    fn is_on_nfs_mount(path: &Path) -> bool {
        use std::ffi::CString;
        use std::mem;
        use std::os::unix::prelude::*;
        use libc;

        let path = match CString::new(path.as_os_str().as_bytes()) {
            Ok(path) => path,
            Err(_) => return false,
        };

        unsafe {
            let mut buf: libc::statfs = mem::zeroed();
            let r = libc::statfs(path.as_ptr(), &mut buf);

            r == 0 && buf.f_type == libc::NFS_SUPER_MAGIC
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn is_on_nfs_mount(_path: &Path) -> bool {
        false
    }
}

fn create_dir_all(path: &Path) -> io::Result<()> {
    match create_dir(path) {
        Ok(()) => return Ok(()),
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                if let Some(p) = path.parent() {
                    return create_dir_all(p).and_then(|()| create_dir(path))
                }
            }
            Err(e)
        }
    }
}

fn create_dir(path: &Path) -> io::Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}
