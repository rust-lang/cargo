//! Various utilities for working with files and paths.

use anyhow::{Context, Result};
use filetime::FileTime;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
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
    env::join_paths(paths.iter())
        .with_context(|| {
            let paths = paths.iter().map(Path::new).collect::<Vec<_>>();
            format!("failed to join path array: {:?}", paths)
        })
        .with_context(|| {
            format!(
                "failed to join search paths together\n\
                     Does ${} have an unterminated quote character?",
                env
            )
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
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
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
                // PATH may have a component like "." in it, so we still need to
                // canonicalize.
                return Ok(candidate.canonicalize()?);
            }
        }

        anyhow::bail!("no executable for `{}` found in PATH", exec.display())
    } else {
        Ok(exec.canonicalize()?)
    }
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
    let meta =
        fs::metadata(path).with_context(|| format!("failed to stat `{}`", path.display()))?;
    Ok(FileTime::from_last_modification_time(&meta))
}

/// Returns the maximum mtime of the given path, recursing into
/// subdirectories, and following symlinks.
pub fn mtime_recursive(path: &Path) -> Result<FileTime> {
    let meta =
        fs::metadata(path).with_context(|| format!("failed to stat `{}`", path.display()))?;
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
                log::debug!("failed to determine mtime while walking directory: {}", e);
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
                        log::debug!(
                            "failed to determine mtime while fetching symlink metdata of {}: {}",
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
                        log::debug!(
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
                        log::debug!(
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
    log::debug!("invocation time for {:?} is {}", path, ft);
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

/// Recursively remove all files and directories at the given directory.
///
/// This does *not* follow symlinks.
pub fn remove_dir_all<P: AsRef<Path>>(p: P) -> Result<()> {
    _remove_dir_all(p.as_ref())
}

fn _remove_dir_all(p: &Path) -> Result<()> {
    if p.symlink_metadata()
        .with_context(|| format!("could not get metadata for `{}` to remove", p.display()))?
        .file_type()
        .is_symlink()
    {
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
pub fn remove_file<P: AsRef<Path>>(p: P) -> Result<()> {
    _remove_file(p.as_ref())
}

fn _remove_file(p: &Path) -> Result<()> {
    let mut err = match fs::remove_file(p) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    if err.kind() == io::ErrorKind::PermissionDenied && set_not_readonly(p).unwrap_or(false) {
        match fs::remove_file(p) {
            Ok(()) => return Ok(()),
            Err(e) => err = e,
        }
    }

    Err(err).with_context(|| format!("failed to remove file `{}`", p.display()))?;
    Ok(())
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
    log::debug!("linking {} to {}", src.display(), dst.display());
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
        #[cfg(target_os = "redox")]
        use std::os::redox::fs::symlink;
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
    } else if env::var_os("__CARGO_COPY_DONT_LINK_DO_NOT_USE_THIS").is_some() {
        // This is a work-around for a bug in macOS 10.15. When running on
        // APFS, there seems to be a strange race condition with
        // Gatekeeper where it will forcefully kill a process launched via
        // `cargo run` with SIGKILL. Copying seems to avoid the problem.
        // This shouldn't affect anyone except Cargo's test suite because
        // it is very rare, and only seems to happen under heavy load and
        // rapidly creating lots of executables and running them.
        // See https://github.com/rust-lang/cargo/issues/7821 for the
        // gory details.
        fs::copy(src, dst).map(|_| ())
    } else {
        fs::hard_link(src, dst)
    };
    link_result
        .or_else(|err| {
            log::debug!("link failed {}. falling back to fs::copy", err);
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
        Ok(()) => log::debug!("set file mtime {} to {}", path.display(), time),
        Err(e) => log::warn!(
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
pub fn strip_prefix_canonical<P: AsRef<Path>>(
    path: P,
    base: P,
) -> Result<PathBuf, std::path::StripPrefixError> {
    // Not all filesystems support canonicalize. Just ignore if it doesn't work.
    let safe_canonicalize = |path: &Path| match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("cannot canonicalize {:?}: {:?}", path, e);
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
    // We do this in two steps (first create a temporary directory and exlucde
    // it from backups, then rename it to the desired name. If we created the
    // directory directly where it should be and then excluded it from backups
    // we would risk a situation where cargo is interrupted right after the directory
    // creation but before the exclusion the the directory would remain non-excluded from
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
    // rename() and suddently the directory (which didn't exist a moment earlier) exists
    // we can infer from it it's another cargo process doing work.
    if let Err(e) = fs::rename(tempdir.path(), path) {
        if !path.exists() {
            return Err(anyhow::Error::from(e));
        }
    }
    Ok(())
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
    let _ = std::fs::write(
        path.join("CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cargo.
# For information about cache directory tags see https://bford.info/cachedir/
",
    );
    // Similarly to exclude_from_time_machine() we ignore errors here as it's an optional feature.
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
        use winapi::um::fileapi::{GetFileAttributesW, SetFileAttributesW};
        use winapi::um::winnt::FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;

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
