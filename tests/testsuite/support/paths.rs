use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use filetime::{self, FileTime};
use lazy_static::lazy_static;

static CARGO_INTEGRATION_TEST_DIR: &'static str = "cit";

lazy_static! {
    static ref GLOBAL_ROOT: PathBuf = {
        let mut path = t!(env::current_exe());
        path.pop(); // chop off exe name
        path.pop(); // chop off 'debug'

        // If `cargo test` is run manually then our path looks like
        // `target/debug/foo`, in which case our `path` is already pointing at
        // `target`. If, however, `cargo test --target $target` is used then the
        // output is `target/$target/debug/foo`, so our path is pointing at
        // `target/$target`. Here we conditionally pop the `$target` name.
        if path.file_name().and_then(|s| s.to_str()) != Some("target") {
            path.pop();
        }

        path.push(CARGO_INTEGRATION_TEST_DIR);
        path.mkdir_p();
        path
    };

    static ref TEST_ROOTS: Mutex<HashMap<String, PathBuf>> = Default::default();
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
    static TEST_ID: RefCell<Option<usize>> = RefCell::new(None);
}

pub struct TestIdGuard {
    _private: (),
}

pub fn init_root() -> TestIdGuard {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    TEST_ID.with(|n| *n.borrow_mut() = Some(id));

    let guard = TestIdGuard { _private: () };

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

pub fn root() -> PathBuf {
    let id = TEST_ID.with(|n| {
        n.borrow().expect(
            "Tests must use the `#[cargo_test]` attribute in \
             order to be able to use the crate root.",
        )
    });
    GLOBAL_ROOT.join(&format!("t{}", id))
}

pub fn home() -> PathBuf {
    let mut path = root();
    path.push("home");
    path.mkdir_p();
    path
}

pub trait CargoPathExt {
    fn rm_rf(&self);
    fn mkdir_p(&self);

    fn move_into_the_past(&self) {
        self.move_in_time(|sec, nsec| (sec - 3600, nsec))
    }

    fn move_into_the_future(&self) {
        self.move_in_time(|sec, nsec| (sec + 3600, nsec))
    }

    fn move_in_time<F>(&self, travel_amount: F)
    where
        F: Fn(i64, u32) -> (i64, u32);

    fn is_symlink(&self) -> bool;
}

impl CargoPathExt for Path {
    /* Technically there is a potential race condition, but we don't
     * care all that much for our tests
     */
    fn rm_rf(&self) {
        if !self.exists() {
            return;
        }

        for file in t!(fs::read_dir(self)) {
            let file = t!(file);
            if file.file_type().map(|m| m.is_dir()).unwrap_or(false) {
                file.path().rm_rf();
            } else {
                // On windows we can't remove a readonly file, and git will
                // often clone files as readonly. As a result, we have some
                // special logic to remove readonly files on windows.
                do_op(&file.path(), "remove file", |p| fs::remove_file(p));
            }
        }
        do_op(self, "remove dir", |p| fs::remove_dir(p));
    }

    fn mkdir_p(&self) {
        fs::create_dir_all(self)
            .unwrap_or_else(|e| panic!("failed to mkdir_p {}: {}", self.display(), e))
    }

    fn move_in_time<F>(&self, travel_amount: F)
    where
        F: Fn(i64, u32) -> ((i64, u32)),
    {
        if self.is_file() {
            time_travel(self, &travel_amount);
        } else {
            recurse(self, &self.join("target"), &travel_amount);
        }

        fn recurse<F>(p: &Path, bad: &Path, travel_amount: &F)
        where
            F: Fn(i64, u32) -> ((i64, u32)),
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
            F: Fn(i64, u32) -> ((i64, u32)),
        {
            let stat = t!(path.metadata());

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

    fn is_symlink(&self) -> bool {
        fs::symlink_metadata(self)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
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
