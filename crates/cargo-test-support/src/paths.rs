//! Access common paths and manipulate the filesystem

use filetime::FileTime;
use itertools::Itertools;
use walkdir::WalkDir;

use std::cell::RefCell;
use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::compare::assert_e2e;
use crate::compare::match_contains;

static CARGO_INTEGRATION_TEST_DIR: &str = "cit";

static GLOBAL_ROOT: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// This is used when running cargo is pre-CARGO_TARGET_TMPDIR
/// TODO: Remove when `CARGO_TARGET_TMPDIR` grows old enough.
fn global_root_legacy() -> PathBuf {
    let mut path = t!(env::current_exe());
    path.pop(); // chop off exe name
    path.pop(); // chop off "deps"
    path.push("tmp");
    path.mkdir_p();
    path
}

fn set_global_root(tmp_dir: Option<&'static str>) {
    let mut lock = GLOBAL_ROOT
        .get_or_init(|| Default::default())
        .lock()
        .unwrap();
    if lock.is_none() {
        let mut root = match tmp_dir {
            Some(tmp_dir) => PathBuf::from(tmp_dir),
            None => global_root_legacy(),
        };

        root.push(CARGO_INTEGRATION_TEST_DIR);
        *lock = Some(root);
    }
}

/// Path to the parent directory of all test [`root`]s
///
/// ex: `$CARGO_TARGET_TMPDIR/cit`
pub fn global_root() -> PathBuf {
    let lock = GLOBAL_ROOT
        .get_or_init(|| Default::default())
        .lock()
        .unwrap();
    match lock.as_ref() {
        Some(p) => p.clone(),
        None => unreachable!("GLOBAL_ROOT not set yet"),
    }
}

// We need to give each test a unique id. The test name could serve this
// purpose, but the `test` crate doesn't have a way to obtain the current test
// name.[*] Instead, we used the `cargo-test-macro` crate to automatically
// insert an init function for each test that sets the test name in a thread
// local variable.
//
// [*] It does set the thread name, but only when running concurrently. If not
// running concurrently, all tests are run on the main thread.
thread_local! {
    static TEST_ID: RefCell<Option<usize>> = const { RefCell::new(None) };
}

/// See [`init_root`]
pub struct TestIdGuard {
    _private: (),
}

/// For test harnesses like [`crate::cargo_test`]
pub fn init_root(tmp_dir: Option<&'static str>) -> TestIdGuard {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    TEST_ID.with(|n| *n.borrow_mut() = Some(id));

    let guard = TestIdGuard { _private: () };

    set_global_root(tmp_dir);
    let r = root();
    r.rm_rf();
    r.mkdir_p();

    guard
}

impl Drop for TestIdGuard {
    fn drop(&mut self) {
        TEST_ID.with(|n| *n.borrow_mut() = None);
    }
}

/// Path to the test's filesystem scratchpad
///
/// ex: `$CARGO_TARGET_TMPDIR/cit/t0`
pub fn root() -> PathBuf {
    let id = TEST_ID.with(|n| {
        n.borrow().expect(
            "Tests must use the `#[cargo_test]` attribute in \
             order to be able to use the crate root.",
        )
    });

    let mut root = global_root();
    root.push(&format!("t{}", id));
    root
}

/// Path to the current test's `$HOME`
///
/// ex: `$CARGO_TARGET_TMPDIR/cit/t0/home`
pub fn home() -> PathBuf {
    let mut path = root();
    path.push("home");
    path.mkdir_p();
    path
}

/// Path to the current test's `$CARGO_HOME`
///
/// ex: `$CARGO_TARGET_TMPDIR/cit/t0/home/.cargo`
pub fn cargo_home() -> PathBuf {
    home().join(".cargo")
}

/// Common path and file operations
pub trait CargoPathExt {
    fn to_url(&self) -> url::Url;

    fn rm_rf(&self);
    fn mkdir_p(&self);

    /// Returns a list of all files and directories underneath the given
    /// directory, recursively, including the starting path.
    fn ls_r(&self) -> Vec<PathBuf>;

    fn move_into_the_past(&self) {
        self.move_in_time(|sec, nsec| (sec - 3600, nsec))
    }

    fn move_into_the_future(&self) {
        self.move_in_time(|sec, nsec| (sec + 3600, nsec))
    }

    fn move_in_time<F>(&self, travel_amount: F)
    where
        F: Fn(i64, u32) -> (i64, u32);

    fn assert_build_dir_layout(&self, expected: impl snapbox::IntoData);

    fn assert_dir_layout(&self, expected: impl snapbox::IntoData, ignored_path_patterns: &[String]);
}

impl CargoPathExt for Path {
    fn to_url(&self) -> url::Url {
        url::Url::from_file_path(self).ok().unwrap()
    }

    fn rm_rf(&self) {
        let meta = match self.symlink_metadata() {
            Ok(meta) => meta,
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    return;
                }
                panic!("failed to remove {:?}, could not read: {:?}", self, e);
            }
        };
        // There is a race condition between fetching the metadata and
        // actually performing the removal, but we don't care all that much
        // for our tests.
        if meta.is_dir() {
            if let Err(e) = fs::remove_dir_all(self) {
                panic!("failed to remove {:?}: {:?}", self, e)
            }
        } else if let Err(e) = fs::remove_file(self) {
            panic!("failed to remove {:?}: {:?}", self, e)
        }
    }

    fn mkdir_p(&self) {
        fs::create_dir_all(self)
            .unwrap_or_else(|e| panic!("failed to mkdir_p {}: {}", self.display(), e))
    }

    fn ls_r(&self) -> Vec<PathBuf> {
        walkdir::WalkDir::new(self)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.map(|e| e.path().to_owned()).ok())
            .collect()
    }

    fn move_in_time<F>(&self, travel_amount: F)
    where
        F: Fn(i64, u32) -> (i64, u32),
    {
        if self.is_file() {
            time_travel(self, &travel_amount);
        } else {
            recurse(self, &self.join("target"), &travel_amount);
        }

        fn recurse<F>(p: &Path, bad: &Path, travel_amount: &F)
        where
            F: Fn(i64, u32) -> (i64, u32),
        {
            if p.is_file() {
                time_travel(p, travel_amount)
            } else if !p.starts_with(bad) {
                for f in t!(fs::read_dir(p)) {
                    let f = t!(f).path();
                    recurse(&f, bad, travel_amount);
                }
            }
        }

        fn time_travel<F>(path: &Path, travel_amount: &F)
        where
            F: Fn(i64, u32) -> (i64, u32),
        {
            let stat = t!(path.symlink_metadata());

            let mtime = FileTime::from_last_modification_time(&stat);

            let (sec, nsec) = travel_amount(mtime.unix_seconds(), mtime.nanoseconds());
            let newtime = FileTime::from_unix_time(sec, nsec);

            // Sadly change_file_times has a failure mode where a readonly file
            // cannot have its times changed on windows.
            do_op(path, "set file times", |path| {
                filetime::set_file_times(path, newtime, newtime)
            });
        }
    }

    #[track_caller]
    fn assert_build_dir_layout(&self, expected: impl snapbox::IntoData) {
        // We call `unordered()` here to because the build-dir has some scenarios that make
        // consistent ordering not possible.
        // Notably:
        // 1. Binaries with `.exe` on Windows causing the ordering to change with the dep-info `.d`
        //    file.
        // 2. Directories with hashes are often reordered differently by platform.
        self.assert_dir_layout(expected.unordered(), &build_dir_ignored_path_patterns());
    }

    #[track_caller]
    fn assert_dir_layout(
        &self,
        expected: impl snapbox::IntoData,
        ignored_path_patterns: &[String],
    ) {
        let assert = assert_e2e();
        let actual = WalkDir::new(self)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_string_lossy().into_owned())
            .filter(|file| {
                for ignored in ignored_path_patterns {
                    if match_contains(&ignored, file, &assert.redactions()).is_ok() {
                        return false;
                    }
                }
                return true;
            })
            .join("\n");

        assert.eq(format!("{actual}\n"), expected);
    }
}

impl CargoPathExt for PathBuf {
    fn to_url(&self) -> url::Url {
        self.as_path().to_url()
    }

    fn rm_rf(&self) {
        self.as_path().rm_rf()
    }
    fn mkdir_p(&self) {
        self.as_path().mkdir_p()
    }

    fn ls_r(&self) -> Vec<PathBuf> {
        self.as_path().ls_r()
    }

    fn move_in_time<F>(&self, travel_amount: F)
    where
        F: Fn(i64, u32) -> (i64, u32),
    {
        self.as_path().move_in_time(travel_amount)
    }

    #[track_caller]
    fn assert_build_dir_layout(&self, expected: impl snapbox::IntoData) {
        self.as_path().assert_build_dir_layout(expected);
    }

    #[track_caller]
    fn assert_dir_layout(
        &self,
        expected: impl snapbox::IntoData,
        ignored_path_patterns: &[String],
    ) {
        self.as_path()
            .assert_dir_layout(expected, ignored_path_patterns);
    }
}

fn do_op<F>(path: &Path, desc: &str, mut f: F)
where
    F: FnMut(&Path) -> io::Result<()>,
{
    match f(path) {
        Ok(()) => {}
        Err(ref e) if e.kind() == ErrorKind::PermissionDenied => {
            let mut p = t!(path.metadata()).permissions();
            p.set_readonly(false);
            t!(fs::set_permissions(path, p));

            // Unix also requires the parent to not be readonly for example when
            // removing files
            let parent = path.parent().unwrap();
            let mut p = t!(parent.metadata()).permissions();
            p.set_readonly(false);
            t!(fs::set_permissions(parent, p));

            f(path).unwrap_or_else(|e| {
                panic!("failed to {} {}: {}", desc, path.display(), e);
            })
        }
        Err(e) => {
            panic!("failed to {} {}: {}", desc, path.display(), e);
        }
    }
}

/// The paths to ignore when [`CargoPathExt::assert_build_dir_layout`] is called
fn build_dir_ignored_path_patterns() -> Vec<String> {
    vec![
        // Ignore MacOS debug symbols as there are many files/directories that would clutter up
        // tests few not a lot of benefit.
        "[..].dSYM/[..]",
        // Ignore Windows debub symbols files (.pdb)
        "[..].pdb",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

/// Get the filename for a library.
///
/// `kind` should be one of:
/// - `lib`
/// - `rlib`
/// - `staticlib`
/// - `dylib`
/// - `proc-macro`
///
/// # Examples
/// ```
/// # use cargo_test_support::paths::get_lib_filename;
/// get_lib_filename("foo", "dylib");
/// ```
/// would return:
/// - macOS: `"libfoo.dylib"`
/// - Windows: `"foo.dll"`
/// - Unix: `"libfoo.so"`
pub fn get_lib_filename(name: &str, kind: &str) -> String {
    let prefix = get_lib_prefix(kind);
    let extension = get_lib_extension(kind);
    format!("{}{}.{}", prefix, name, extension)
}

/// See [`get_lib_filename`] for more details
pub fn get_lib_prefix(kind: &str) -> &str {
    match kind {
        "lib" | "rlib" => "lib",
        "staticlib" | "dylib" | "proc-macro" => {
            if cfg!(windows) {
                ""
            } else {
                "lib"
            }
        }
        _ => unreachable!(),
    }
}

/// See [`get_lib_filename`] for more details
pub fn get_lib_extension(kind: &str) -> &str {
    match kind {
        "lib" | "rlib" => "rlib",
        "staticlib" => {
            if cfg!(windows) {
                "lib"
            } else {
                "a"
            }
        }
        "dylib" | "proc-macro" => {
            if cfg!(windows) {
                "dll"
            } else if cfg!(target_os = "macos") {
                "dylib"
            } else {
                "so"
            }
        }
        _ => unreachable!(),
    }
}

/// Path to `rustc`s sysroot
pub fn sysroot() -> String {
    let output = Command::new("rustc")
        .arg("--print=sysroot")
        .output()
        .expect("rustc to run");
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    sysroot.trim().to_string()
}

/// Returns true if names such as aux.* are allowed.
///
/// Traditionally, Windows did not allow a set of file names (see `is_windows_reserved`
/// for a list). More recent versions of Windows have relaxed this restriction. This test
/// determines whether we are running in a mode that allows Windows reserved names.
#[cfg(windows)]
pub fn windows_reserved_names_are_allowed() -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use windows_sys::Win32::Storage::FileSystem::GetFullPathNameW;

    let test_file_name: Vec<_> = OsStr::new("aux.rs").encode_wide().chain([0]).collect();

    let buffer_length =
        unsafe { GetFullPathNameW(test_file_name.as_ptr(), 0, ptr::null_mut(), ptr::null_mut()) };

    if buffer_length == 0 {
        // This means the call failed, so we'll conservatively assume reserved names are not allowed.
        return false;
    }

    let mut buffer = vec![0u16; buffer_length as usize];

    let result = unsafe {
        GetFullPathNameW(
            test_file_name.as_ptr(),
            buffer_length,
            buffer.as_mut_ptr(),
            ptr::null_mut(),
        )
    };

    if result == 0 {
        // Once again, conservatively assume reserved names are not allowed if the
        // GetFullPathNameW call failed.
        return false;
    }

    // Under the old rules, a file name like aux.rs would get converted into \\.\aux, so
    // we detect this case by checking if the string starts with \\.\
    //
    // Otherwise, the filename will be something like C:\Users\Foo\Documents\aux.rs
    let prefix: Vec<_> = OsStr::new("\\\\.\\").encode_wide().collect();
    if buffer.starts_with(&prefix) {
        false
    } else {
        true
    }
}
