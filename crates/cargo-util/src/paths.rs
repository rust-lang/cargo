//! Various utilities for working with files and paths.

use anyhow::{Context, Result};
use filetime::FileTime;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::iter;
use std::path::{Component, Path, PathBuf};
use tempfile::Builder as TempFileBuilder;

/// Joins paths into a string suitable for the `PATH` environment variable.
///
/// This is equivalent to [`std::env::join_paths`], but includes a more
/// detailed error message. The given `env` argument is the name of the
/// environment variable this is will be used for, which is included in the
/// error message.
pub fn join_paths<T: AsRef<OsStr>>(paths: &[T], env: &str) -> Result<OsString> {
    env::join_paths(paths.iter()).with_context(|| {
        let mut message = format!(
            "failed to join paths from `${env}` together\n\n\
             Check if any of path segments listed below contain an \
             unterminated quote character or path separator:"
        );
        for path in paths {
            use std::fmt::Write;
            write!(&mut message, "\n    {:?}", Path::new(path)).unwrap();
        }

        message
    })
}

/// Returns the name of the environment variable used for searching for
/// dynamic libraries.
pub fn dylib_path_envvar() -> &'static str {
    if cfg!(windows) {
        "PATH"
    } else if cfg!(target_os = "macos") {
        // When loading and linking a dynamic library or bundle, dlopen
        // searches in LD_LIBRARY_PATH, DYLD_LIBRARY_PATH, PWD, and
        // DYLD_FALLBACK_LIBRARY_PATH.
        // In the Mach-O format, a dynamic library has an "install path."
        // Clients linking against the library record this path, and the
        // dynamic linker, dyld, uses it to locate the library.
        // dyld searches DYLD_LIBRARY_PATH *before* the install path.
        // dyld searches DYLD_FALLBACK_LIBRARY_PATH only if it cannot
        // find the library in the install path.
        // Setting DYLD_LIBRARY_PATH can easily have unintended
        // consequences.
        //
        // Also, DYLD_LIBRARY_PATH appears to have significant performance
        // penalty starting in 10.13. Cargo's testsuite ran more than twice as
        // slow with it on CI.
        "DYLD_FALLBACK_LIBRARY_PATH"
    } else if cfg!(target_os = "aix") {
        "LIBPATH"
    } else {
        "LD_LIBRARY_PATH"
    }
}

/// Returns a list of directories that are searched for dynamic libraries.
///
/// Note that some operating systems will have defaults if this is empty that
/// will need to be dealt with.
pub fn dylib_path() -> Vec<PathBuf> {
    match env::var_os(dylib_path_envvar()) {
        Some(var) => env::split_paths(&var).collect(),
        None => Vec::new(),
    }
}

/// Normalize a path, removing things like `.` and `..`.
///
/// CAUTION: This does not resolve symlinks (unlike
/// [`std::fs::canonicalize`]). This may cause incorrect or surprising
/// behavior at times. This should be used carefully. Unfortunately,
/// [`std::fs::canonicalize`] can be hard to use correctly, since it can often
/// fail, or on Windows returns annoying device paths. This is a problem Cargo
/// needs to improve on.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(Component::RootDir);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if ret.ends_with(Component::ParentDir) {
                    ret.push(Component::ParentDir);
                } else {
                    let popped = ret.pop();
                    if !popped && !ret.has_root() {
                        ret.push(Component::ParentDir);
                    }
                }
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

/// Returns the absolute path of where the given executable is located based
/// on searching the `PATH` environment variable.
///
/// Returns an error if it cannot be found.
pub fn resolve_executable(exec: &Path) -> Result<PathBuf> {
    if exec.components().count() == 1 {
        let paths = env::var_os("PATH").ok_or_else(|| anyhow::format_err!("no PATH"))?;
        let candidates = env::split_paths(&paths).flat_map(|path| {
            let candidate = path.join(&exec);
            let with_exe = if env::consts::EXE_EXTENSION.is_empty() {
                None
            } else {
                Some(candidate.with_extension(env::consts::EXE_EXTENSION))
            };
            iter::once(candidate).chain(with_exe)
        });
        for candidate in candidates {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        anyhow::bail!("no executable for `{}` found in PATH", exec.display())
    } else {
        Ok(exec.into())
    }
}

/// Returns metadata for a file (follows symlinks).
///
/// Equivalent to [`std::fs::metadata`] with better error messages.
pub fn metadata<P: AsRef<Path>>(path: P) -> Result<Metadata> {
    let path = path.as_ref();
    std::fs::metadata(path)
        .with_context(|| format!("failed to load metadata for path `{}`", path.display()))
}

/// Returns metadata for a file without following symlinks.
///
/// Equivalent to [`std::fs::metadata`] with better error messages.
pub fn symlink_metadata<P: AsRef<Path>>(path: P) -> Result<Metadata> {
    let path = path.as_ref();
    std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to load metadata for path `{}`", path.display()))
}

/// Reads a file to a string.
///
/// Equivalent to [`std::fs::read_to_string`] with better error messages.
pub fn read(path: &Path) -> Result<String> {
    match String::from_utf8(read_bytes(path)?) {
        Ok(s) => Ok(s),
        Err(_) => anyhow::bail!("path at `{}` was not valid utf-8", path.display()),
    }
}

/// Reads a file into a bytes vector.
///
/// Equivalent to [`std::fs::read`] with better error messages.
pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("failed to read `{}`", path.display()))
}

/// Writes a file to disk.
///
/// Equivalent to [`std::fs::write`] with better error messages.
pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();
    fs::write(path, contents.as_ref())
        .with_context(|| format!("failed to write `{}`", path.display()))
}

/// Writes a file to disk atomically.
///
/// This uses `tempfile::persist` to accomplish atomic writes.
/// If the path is a symlink, it will follow the symlink and write to the actual target.
pub fn write_atomic<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();

    // Check if the path is a symlink and follow it if it is
    let resolved_path;
    let path = if path.is_symlink() {
        resolved_path = fs::read_link(path)
            .with_context(|| format!("failed to read symlink at `{}`", path.display()))?;
        &resolved_path
    } else {
        path
    };

    // On unix platforms, get the permissions of the original file. Copy only the user/group/other
    // read/write/execute permission bits. The tempfile lib defaults to an initial mode of 0o600,
    // and we'll set the proper permissions after creating the file.
    #[cfg(unix)]
    let perms = path.metadata().ok().map(|meta| {
        use std::os::unix::fs::PermissionsExt;

        // these constants are u16 on macOS
        let mask = u32::from(libc::S_IRWXU | libc::S_IRWXG | libc::S_IRWXO);
        let mode = meta.permissions().mode() & mask;

        std::fs::Permissions::from_mode(mode)
    });

    let mut tmp = TempFileBuilder::new()
        .prefix(path.file_name().unwrap())
        .tempfile_in(path.parent().unwrap())?;
    tmp.write_all(contents.as_ref())?;

    // On unix platforms, set the permissions on the newly created file. We can use fchmod (called
    // by the std lib; subject to change) which ignores the umask so that the new file has the same
    // permissions as the old file.
    #[cfg(unix)]
    if let Some(perms) = perms {
        tmp.as_file().set_permissions(perms)?;
    }

    tmp.persist(path)?;
    Ok(())
}

/// Equivalent to [`write()`], but does not write anything if the file contents
/// are identical to the given contents.
pub fn write_if_changed<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    (|| -> Result<()> {
        let contents = contents.as_ref();
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        let mut orig = Vec::new();
        f.read_to_end(&mut orig)?;
        if orig != contents {
            f.set_len(0)?;
            f.seek(io::SeekFrom::Start(0))?;
            f.write_all(contents)?;
        }
        Ok(())
    })()
    .with_context(|| format!("failed to write `{}`", path.as_ref().display()))?;
    Ok(())
}

/// Equivalent to [`write()`], but appends to the end instead of replacing the
/// contents.
pub fn append(path: &Path, contents: &[u8]) -> Result<()> {
    (|| -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(path)?;

        f.write_all(contents)?;
        Ok(())
    })()
    .with_context(|| format!("failed to write `{}`", path.display()))?;
    Ok(())
}

/// Creates a new file.
pub fn create<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::create(path).with_context(|| format!("failed to create file `{}`", path.display()))
}

/// Opens an existing file.
pub fn open<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::open(path).with_context(|| format!("failed to open file `{}`", path.display()))
}

/// Returns the last modification time of a file.
pub fn mtime(path: &Path) -> Result<FileTime> {
    let meta = metadata(path)?;
    Ok(FileTime::from_last_modification_time(&meta))
}

/// Returns the maximum mtime of the given path, recursing into
/// subdirectories, and following symlinks.
pub fn mtime_recursive(path: &Path) -> Result<FileTime> {
    let meta = metadata(path)?;
    if !meta.is_dir() {
        return Ok(FileTime::from_last_modification_time(&meta));
    }
    let max_meta = walkdir::WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                // Ignore errors while walking. If Cargo can't access it, the
                // build script probably can't access it, either.
                tracing::debug!("failed to determine mtime while walking directory: {}", e);
                None
            }
        })
        .filter_map(|e| {
            if e.path_is_symlink() {
                // Use the mtime of both the symlink and its target, to
                // handle the case where the symlink is modified to a
                // different target.
                let sym_meta = match std::fs::symlink_metadata(e.path()) {
                    Ok(m) => m,
                    Err(err) => {
                        // I'm not sure when this is really possible (maybe a
                        // race with unlinking?). Regardless, if Cargo can't
                        // read it, the build script probably can't either.
                        tracing::debug!(
                            "failed to determine mtime while fetching symlink metadata of {}: {}",
                            e.path().display(),
                            err
                        );
                        return None;
                    }
                };
                let sym_mtime = FileTime::from_last_modification_time(&sym_meta);
                // Walkdir follows symlinks.
                match e.metadata() {
                    Ok(target_meta) => {
                        let target_mtime = FileTime::from_last_modification_time(&target_meta);
                        Some(sym_mtime.max(target_mtime))
                    }
                    Err(err) => {
                        // Can't access the symlink target. If Cargo can't
                        // access it, the build script probably can't access
                        // it either.
                        tracing::debug!(
                            "failed to determine mtime of symlink target for {}: {}",
                            e.path().display(),
                            err
                        );
                        Some(sym_mtime)
                    }
                }
            } else {
                let meta = match e.metadata() {
                    Ok(m) => m,
                    Err(err) => {
                        // I'm not sure when this is really possible (maybe a
                        // race with unlinking?). Regardless, if Cargo can't
                        // read it, the build script probably can't either.
                        tracing::debug!(
                            "failed to determine mtime while fetching metadata of {}: {}",
                            e.path().display(),
                            err
                        );
                        return None;
                    }
                };
                Some(FileTime::from_last_modification_time(&meta))
            }
        })
        .max()
        // or_else handles the case where there are no files in the directory.
        .unwrap_or_else(|| FileTime::from_last_modification_time(&meta));
    Ok(max_meta)
}

/// Record the current time on the filesystem (using the filesystem's clock)
/// using a file at the given directory. Returns the current time.
pub fn set_invocation_time(path: &Path) -> Result<FileTime> {
    // note that if `FileTime::from_system_time(SystemTime::now());` is determined to be sufficient,
    // then this can be removed.
    let timestamp = path.join("invoked.timestamp");
    write(
        &timestamp,
        "This file has an mtime of when this was started.",
    )?;
    let ft = mtime(&timestamp)?;
    tracing::debug!("invocation time for {:?} is {}", path, ft);
    Ok(ft)
}

/// Converts a path to UTF-8 bytes.
pub fn path2bytes(path: &Path) -> Result<&[u8]> {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::*;
        Ok(path.as_os_str().as_bytes())
    }
    #[cfg(windows)]
    {
        match path.as_os_str().to_str() {
            Some(s) => Ok(s.as_bytes()),
            None => Err(anyhow::format_err!(
                "invalid non-unicode path: {}",
                path.display()
            )),
        }
    }
}

/// Converts UTF-8 bytes to a path.
pub fn bytes2path(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::*;
        Ok(PathBuf::from(OsStr::from_bytes(bytes)))
    }
    #[cfg(windows)]
    {
        use std::str;
        match str::from_utf8(bytes) {
            Ok(s) => Ok(PathBuf::from(s)),
            Err(..) => Err(anyhow::format_err!("invalid non-unicode path")),
        }
    }
}

/// Returns an iterator that walks up the directory hierarchy towards the root.
///
/// Each item is a [`Path`]. It will start with the given path, finishing at
/// the root. If the `stop_root_at` parameter is given, it will stop at the
/// given path (which will be the last item).
pub fn ancestors<'a>(path: &'a Path, stop_root_at: Option<&Path>) -> PathAncestors<'a> {
    PathAncestors::new(path, stop_root_at)
}

pub struct PathAncestors<'a> {
    current: Option<&'a Path>,
    stop_at: Option<PathBuf>,
}

impl<'a> PathAncestors<'a> {
    fn new(path: &'a Path, stop_root_at: Option<&Path>) -> PathAncestors<'a> {
        let stop_at = env::var("__CARGO_TEST_ROOT")
            .ok()
            .map(PathBuf::from)
            .or_else(|| stop_root_at.map(|p| p.to_path_buf()));
        PathAncestors {
            current: Some(path),
            //HACK: avoid reading `~/.cargo/config` when testing Cargo itself.
            stop_at,
        }
    }
}

impl<'a> Iterator for PathAncestors<'a> {
    type Item = &'a Path;

    fn next(&mut self) -> Option<&'a Path> {
        if let Some(path) = self.current {
            self.current = path.parent();

            if let Some(ref stop_at) = self.stop_at {
                if path == stop_at {
                    self.current = None;
                }
            }

            Some(path)
        } else {
            None
        }
    }
}

/// Equivalent to [`std::fs::create_dir_all`] with better error messages.
pub fn create_dir_all(p: impl AsRef<Path>) -> Result<()> {
    _create_dir_all(p.as_ref())
}

fn _create_dir_all(p: &Path) -> Result<()> {
    fs::create_dir_all(p)
        .with_context(|| format!("failed to create directory `{}`", p.display()))?;
    Ok(())
}

/// Equivalent to [`std::fs::remove_dir_all`] with better error messages.
///
/// This does *not* follow symlinks.
pub fn remove_dir_all<P: AsRef<Path>>(p: P) -> Result<()> {
    _remove_dir_all(p.as_ref()).or_else(|prev_err| {
        // `std::fs::remove_dir_all` is highly specialized for different platforms
        // and may be more reliable than a simple walk. We try the walk first in
        // order to report more detailed errors.
        fs::remove_dir_all(p.as_ref()).with_context(|| {
            format!(
                "{:?}\n\nError: failed to remove directory `{}`",
                prev_err,
                p.as_ref().display(),
            )
        })
    })
}

fn _remove_dir_all(p: &Path) -> Result<()> {
    if symlink_metadata(p)?.is_symlink() {
        return remove_file(p);
    }
    let entries = p
        .read_dir()
        .with_context(|| format!("failed to read directory `{}`", p.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            remove_dir_all(&path)?;
        } else {
            remove_file(&path)?;
        }
    }
    remove_dir(&p)
}

/// Equivalent to [`std::fs::remove_dir`] with better error messages.
pub fn remove_dir<P: AsRef<Path>>(p: P) -> Result<()> {
    _remove_dir(p.as_ref())
}

fn _remove_dir(p: &Path) -> Result<()> {
    fs::remove_dir(p).with_context(|| format!("failed to remove directory `{}`", p.display()))?;
    Ok(())
}

/// Equivalent to [`std::fs::remove_file`] with better error messages.
///
/// If the file is readonly, this will attempt to change the permissions to
/// force the file to be deleted.
/// On Windows, if the file is a symlink to a directory, this will attempt to remove
/// the symlink itself.
pub fn remove_file<P: AsRef<Path>>(p: P) -> Result<()> {
    _remove_file(p.as_ref())
}

fn _remove_file(p: &Path) -> Result<()> {
    // For Windows, we need to check if the file is a symlink to a directory
    // and remove the symlink itself by calling `remove_dir` instead of
    // `remove_file`.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::FileTypeExt;
        let metadata = symlink_metadata(p)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink_dir() {
            return remove_symlink_dir_with_permission_check(p);
        }
    }

    remove_file_with_permission_check(p)
}

#[cfg(target_os = "windows")]
fn remove_symlink_dir_with_permission_check(p: &Path) -> Result<()> {
    remove_with_permission_check(fs::remove_dir, p)
        .with_context(|| format!("failed to remove symlink dir `{}`", p.display()))
}

fn remove_file_with_permission_check(p: &Path) -> Result<()> {
    remove_with_permission_check(fs::remove_file, p)
        .with_context(|| format!("failed to remove file `{}`", p.display()))
}

fn remove_with_permission_check<F, P>(remove_func: F, p: P) -> io::Result<()>
where
    F: Fn(P) -> io::Result<()>,
    P: AsRef<Path> + Clone,
{
    match remove_func(p.clone()) {
        Ok(()) => Ok(()),
        Err(e) => {
            if e.kind() == io::ErrorKind::PermissionDenied
                && set_not_readonly(p.as_ref()).unwrap_or(false)
            {
                remove_func(p)
            } else {
                Err(e)
            }
        }
    }
}

fn set_not_readonly(p: &Path) -> io::Result<bool> {
    let mut perms = p.metadata()?.permissions();
    if !perms.readonly() {
        return Ok(false);
    }
    perms.set_readonly(false);
    fs::set_permissions(p, perms)?;
    Ok(true)
}

/// Hardlink (file) or symlink (dir) src to dst if possible, otherwise copy it.
///
/// If the destination already exists, it is removed before linking.
pub fn link_or_copy(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    _link_or_copy(src, dst)
}

fn _link_or_copy(src: &Path, dst: &Path) -> Result<()> {
    tracing::debug!("linking {} to {}", src.display(), dst.display());
    if same_file::is_same_file(src, dst).unwrap_or(false) {
        return Ok(());
    }

    // NB: we can't use dst.exists(), as if dst is a broken symlink,
    // dst.exists() will return false. This is problematic, as we still need to
    // unlink dst in this case. symlink_metadata(dst).is_ok() will tell us
    // whether dst exists *without* following symlinks, which is what we want.
    if fs::symlink_metadata(dst).is_ok() {
        remove_file(&dst)?;
    }

    let link_result = if src.is_dir() {
        #[cfg(unix)]
        use std::os::unix::fs::symlink;
        #[cfg(windows)]
        // FIXME: This should probably panic or have a copy fallback. Symlinks
        // are not supported in all windows environments. Currently symlinking
        // is only used for .dSYM directories on macos, but this shouldn't be
        // accidentally relied upon.
        use std::os::windows::fs::symlink_dir as symlink;

        let dst_dir = dst.parent().unwrap();
        let src = if src.starts_with(dst_dir) {
            src.strip_prefix(dst_dir).unwrap()
        } else {
            src
        };
        symlink(src, dst)
    } else {
        if cfg!(target_os = "macos") {
            // There seems to be a race condition with APFS when hard-linking
            // binaries. Gatekeeper does not have signing or hash information
            // stored in kernel when running the process. Therefore killing it.
            // This problem does not appear when copying files as kernel has
            // time to process it. Note that: fs::copy on macos is using
            // CopyOnWrite (syscall fclonefileat) which should be as fast as
            // hardlinking. See these issues for the details:
            //
            // * https://github.com/rust-lang/cargo/issues/7821
            // * https://github.com/rust-lang/cargo/issues/10060
            fs::copy(src, dst).map_or_else(
                |e| {
                    if e.raw_os_error()
                        .map_or(false, |os_err| os_err == 35 /* libc::EAGAIN */)
                    {
                        tracing::info!("copy failed {e:?}. falling back to fs::hard_link");

                        // Working around an issue copying too fast with zfs (probably related to
                        // https://github.com/openzfsonosx/zfs/issues/809)
                        // See https://github.com/rust-lang/cargo/issues/13838
                        fs::hard_link(src, dst)
                    } else {
                        Err(e)
                    }
                },
                |_| Ok(()),
            )
        } else {
            fs::hard_link(src, dst)
        }
    };
    link_result
        .or_else(|err| {
            tracing::debug!("link failed {}. falling back to fs::copy", err);
            fs::copy(src, dst).map(|_| ())
        })
        .with_context(|| {
            format!(
                "failed to link or copy `{}` to `{}`",
                src.display(),
                dst.display()
            )
        })?;
    Ok(())
}

/// Copies a file from one location to another.
///
/// Equivalent to [`std::fs::copy`] with better error messages.
pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<u64> {
    let from = from.as_ref();
    let to = to.as_ref();
    fs::copy(from, to)
        .with_context(|| format!("failed to copy `{}` to `{}`", from.display(), to.display()))
}

/// Changes the filesystem mtime (and atime if possible) for the given file.
///
/// This intentionally does not return an error, as this is sometimes not
/// supported on network filesystems. For the current uses in Cargo, this is a
/// "best effort" approach, and errors shouldn't be propagated.
pub fn set_file_time_no_err<P: AsRef<Path>>(path: P, time: FileTime) {
    let path = path.as_ref();
    match filetime::set_file_times(path, time, time) {
        Ok(()) => tracing::debug!("set file mtime {} to {}", path.display(), time),
        Err(e) => tracing::warn!(
            "could not set mtime of {} to {}: {:?}",
            path.display(),
            time,
            e
        ),
    }
}

/// Strips `base` from `path`.
///
/// This canonicalizes both paths before stripping. This is useful if the
/// paths are obtained in different ways, and one or the other may or may not
/// have been normalized in some way.
pub fn strip_prefix_canonical(
    path: impl AsRef<Path>,
    base: impl AsRef<Path>,
) -> Result<PathBuf, std::path::StripPrefixError> {
    // Not all filesystems support canonicalize. Just ignore if it doesn't work.
    let safe_canonicalize = |path: &Path| match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("cannot canonicalize {:?}: {:?}", path, e);
            path.to_path_buf()
        }
    };
    let canon_path = safe_canonicalize(path.as_ref());
    let canon_base = safe_canonicalize(base.as_ref());
    canon_path.strip_prefix(canon_base).map(|p| p.to_path_buf())
}

/// Creates an excluded from cache directory atomically with its parents as needed.
///
/// The atomicity only covers creating the leaf directory and exclusion from cache. Any missing
/// parent directories will not be created in an atomic manner.
///
/// This function is idempotent and in addition to that it won't exclude ``p`` from cache if it
/// already exists.
pub fn create_dir_all_excluded_from_backups_atomic(p: impl AsRef<Path>) -> Result<()> {
    let path = p.as_ref();
    if path.is_dir() {
        return Ok(());
    }

    let parent = path.parent().unwrap();
    let base = path.file_name().unwrap();
    create_dir_all(parent)?;
    // We do this in two steps (first create a temporary directory and exclude
    // it from backups, then rename it to the desired name. If we created the
    // directory directly where it should be and then excluded it from backups
    // we would risk a situation where cargo is interrupted right after the directory
    // creation but before the exclusion the directory would remain non-excluded from
    // backups because we only perform exclusion right after we created the directory
    // ourselves.
    //
    // We need the tempdir created in parent instead of $TMP, because only then we can be
    // easily sure that rename() will succeed (the new name needs to be on the same mount
    // point as the old one).
    let tempdir = TempFileBuilder::new().prefix(base).tempdir_in(parent)?;
    exclude_from_backups(tempdir.path());
    exclude_from_content_indexing(tempdir.path());
    // Previously std::fs::create_dir_all() (through paths::create_dir_all()) was used
    // here to create the directory directly and fs::create_dir_all() explicitly treats
    // the directory being created concurrently by another thread or process as success,
    // hence the check below to follow the existing behavior. If we get an error at
    // rename() and suddenly the directory (which didn't exist a moment earlier) exists
    // we can infer from it's another cargo process doing work.
    if let Err(e) = fs::rename(tempdir.path(), path) {
        if !path.exists() {
            return Err(anyhow::Error::from(e))
                .with_context(|| format!("failed to create directory `{}`", path.display()));
        }
    }
    Ok(())
}

/// Mark an existing directory as excluded from backups and indexing.
///
/// Errors in marking it are ignored.
pub fn exclude_from_backups_and_indexing(p: impl AsRef<Path>) {
    let path = p.as_ref();
    exclude_from_backups(path);
    exclude_from_content_indexing(path);
}

/// Marks the directory as excluded from archives/backups.
///
/// This is recommended to prevent derived/temporary files from bloating backups. There are two
/// mechanisms used to achieve this right now:
///
/// * A dedicated resource property excluding from Time Machine backups on macOS
/// * CACHEDIR.TAG files supported by various tools in a platform-independent way
fn exclude_from_backups(path: &Path) {
    exclude_from_time_machine(path);
    let file = path.join("CACHEDIR.TAG");
    if !file.exists() {
        let _ = std::fs::write(
            file,
            "Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cargo.
# For information about cache directory tags see https://bford.info/cachedir/
",
        );
        // Similarly to exclude_from_time_machine() we ignore errors here as it's an optional feature.
    }
}

/// Marks the directory as excluded from content indexing.
///
/// This is recommended to prevent the content of derived/temporary files from being indexed.
/// This is very important for Windows users, as the live content indexing significantly slows
/// cargo's I/O operations.
///
/// This is currently a no-op on non-Windows platforms.
fn exclude_from_content_indexing(path: &Path) {
    #[cfg(windows)]
    {
        use std::iter::once;
        use std::os::windows::prelude::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_NOT_CONTENT_INDEXED, GetFileAttributesW, SetFileAttributesW,
        };

        let path: Vec<u16> = path.as_os_str().encode_wide().chain(once(0)).collect();
        unsafe {
            SetFileAttributesW(
                path.as_ptr(),
                GetFileAttributesW(path.as_ptr()) | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
}

#[cfg(not(target_os = "macos"))]
fn exclude_from_time_machine(_: &Path) {}

#[cfg(target_os = "macos")]
/// Marks files or directories as excluded from Time Machine on macOS
fn exclude_from_time_machine(path: &Path) {
    use core_foundation::base::TCFType;
    use core_foundation::{number, string, url};
    use std::ptr;

    // For compatibility with 10.7 a string is used instead of global kCFURLIsExcludedFromBackupKey
    let is_excluded_key: Result<string::CFString, _> = "NSURLIsExcludedFromBackupKey".parse();
    let path = url::CFURL::from_path(path, false);
    if let (Some(path), Ok(is_excluded_key)) = (path, is_excluded_key) {
        unsafe {
            url::CFURLSetResourcePropertyForKey(
                path.as_concrete_TypeRef(),
                is_excluded_key.as_concrete_TypeRef(),
                number::kCFBooleanTrue as *const _,
                ptr::null_mut(),
            );
        }
    }
    // Errors are ignored, since it's an optional feature and failure
    // doesn't prevent Cargo from working
}

#[cfg(test)]
mod tests {
    use super::join_paths;
    use super::normalize_path;
    use super::write;
    use super::write_atomic;

    #[test]
    fn test_normalize_path() {
        let cases = &[
            ("", ""),
            (".", ""),
            (".////./.", ""),
            ("/", "/"),
            ("/..", "/"),
            ("/foo/bar", "/foo/bar"),
            ("/foo/bar/", "/foo/bar"),
            ("/foo/bar/./././///", "/foo/bar"),
            ("/foo/bar/..", "/foo"),
            ("/foo/bar/../..", "/"),
            ("/foo/bar/../../..", "/"),
            ("foo/bar", "foo/bar"),
            ("foo/bar/", "foo/bar"),
            ("foo/bar/./././///", "foo/bar"),
            ("foo/bar/..", "foo"),
            ("foo/bar/../..", ""),
            ("foo/bar/../../..", ".."),
            ("../../foo/bar", "../../foo/bar"),
            ("../../foo/bar/", "../../foo/bar"),
            ("../../foo/bar/./././///", "../../foo/bar"),
            ("../../foo/bar/..", "../../foo"),
            ("../../foo/bar/../..", "../.."),
            ("../../foo/bar/../../..", "../../.."),
        ];
        for (input, expected) in cases {
            let actual = normalize_path(std::path::Path::new(input));
            assert_eq!(actual, std::path::Path::new(expected), "input: {input}");
        }
    }

    #[test]
    fn write_works() {
        let original_contents = "[dependencies]\nfoo = 0.1.0";

        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("Cargo.toml");
        write(&path, original_contents).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, original_contents);
    }
    #[test]
    fn write_atomic_works() {
        let original_contents = "[dependencies]\nfoo = 0.1.0";

        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("Cargo.toml");
        write_atomic(&path, original_contents).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, original_contents);
    }

    #[test]
    #[cfg(unix)]
    fn write_atomic_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let original_perms = std::fs::Permissions::from_mode(u32::from(
            libc::S_IRWXU | libc::S_IRGRP | libc::S_IWGRP | libc::S_IROTH,
        ));

        let tmp = tempfile::Builder::new().tempfile().unwrap();

        // need to set the permissions after creating the file to avoid umask
        tmp.as_file()
            .set_permissions(original_perms.clone())
            .unwrap();

        // after this call, the file at `tmp.path()` will not be the same as the file held by `tmp`
        write_atomic(tmp.path(), "new").unwrap();
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "new");

        let new_perms = std::fs::metadata(tmp.path()).unwrap().permissions();

        let mask = u32::from(libc::S_IRWXU | libc::S_IRWXG | libc::S_IRWXO);
        assert_eq!(original_perms.mode(), new_perms.mode() & mask);
    }

    #[test]
    fn join_paths_lists_paths_on_error() {
        let valid_paths = vec!["/testing/one", "/testing/two"];
        // does not fail on valid input
        let _joined = join_paths(&valid_paths, "TESTING1").unwrap();

        #[cfg(unix)]
        {
            let invalid_paths = vec!["/testing/one", "/testing/t:wo/three"];
            let err = join_paths(&invalid_paths, "TESTING2").unwrap_err();
            assert_eq!(
                err.to_string(),
                "failed to join paths from `$TESTING2` together\n\n\
             Check if any of path segments listed below contain an \
             unterminated quote character or path separator:\
             \n    \"/testing/one\"\
             \n    \"/testing/t:wo/three\"\
             "
            );
        }
        #[cfg(windows)]
        {
            let invalid_paths = vec!["/testing/one", "/testing/t\"wo/three"];
            let err = join_paths(&invalid_paths, "TESTING2").unwrap_err();
            assert_eq!(
                err.to_string(),
                "failed to join paths from `$TESTING2` together\n\n\
             Check if any of path segments listed below contain an \
             unterminated quote character or path separator:\
             \n    \"/testing/one\"\
             \n    \"/testing/t\\\"wo/three\"\
             "
            );
        }
    }

    #[test]
    fn write_atomic_symlink() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target_path = tmpdir.path().join("target.txt");
        let symlink_path = tmpdir.path().join("symlink.txt");

        // Create initial file
        write(&target_path, "initial").unwrap();

        // Create symlink
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target_path, &symlink_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target_path, &symlink_path).unwrap();

        // Write through symlink
        write_atomic(&symlink_path, "updated").unwrap();

        // Verify both paths show the updated content
        assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "updated");
        assert_eq!(std::fs::read_to_string(&symlink_path).unwrap(), "updated");

        // Verify symlink still exists and points to the same target
        assert!(symlink_path.is_symlink());
        assert_eq!(std::fs::read_link(&symlink_path).unwrap(), target_path);
    }

    #[test]
    #[cfg(windows)]
    fn test_remove_symlink_dir() {
        use super::*;
        use std::fs;
        use std::os::windows::fs::symlink_dir;

        let tmpdir = tempfile::tempdir().unwrap();
        let dir_path = tmpdir.path().join("testdir");
        let symlink_path = tmpdir.path().join("symlink");

        fs::create_dir(&dir_path).unwrap();

        symlink_dir(&dir_path, &symlink_path).expect("failed to create symlink");

        assert!(symlink_path.exists());

        assert!(remove_file(symlink_path.clone()).is_ok());

        assert!(!symlink_path.exists());
        assert!(dir_path.exists());
    }

    #[test]
    #[cfg(windows)]
    fn test_remove_symlink_file() {
        use super::*;
        use std::fs;
        use std::os::windows::fs::symlink_file;

        let tmpdir = tempfile::tempdir().unwrap();
        let file_path = tmpdir.path().join("testfile");
        let symlink_path = tmpdir.path().join("symlink");

        fs::write(&file_path, b"test").unwrap();

        symlink_file(&file_path, &symlink_path).expect("failed to create symlink");

        assert!(symlink_path.exists());

        assert!(remove_file(symlink_path.clone()).is_ok());

        assert!(!symlink_path.exists());
        assert!(file_path.exists());
    }
}
