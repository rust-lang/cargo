use std::io::IoResult;
use std::io::fs;
use std::sync::atomics;
use std::{io, os};

use cargo::util::realpath;

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cargo-integration-tests";

local_data_key!(task_id: uint)

static mut NEXT_ID: atomics::AtomicUint = atomics::INIT_ATOMIC_UINT;

pub fn root() -> Path {
    let my_id = *task_id.get().unwrap();
    let path = os::tmpdir().join(CARGO_INTEGRATION_TEST_DIR)
                           .join(format!("test-{}", my_id));
    realpath(&path).unwrap()
}

pub fn home() -> Path {
    root().join("home")
}

pub trait PathExt {
    fn rm_rf(&self) -> IoResult<()>;
    fn mkdir_p(&self) -> IoResult<()>;
}

impl PathExt for Path {
    /* Technically there is a potential race condition, but we don't
     * care all that much for our tests
     */
    fn rm_rf(&self) -> IoResult<()> {
        if self.exists() {
            fs::rmdir_recursive(self)
        }
        else {
            Ok(())
        }
    }

    fn mkdir_p(&self) -> IoResult<()> {
        fs::mkdir_recursive(self, io::UserDir)
    }
}

/**
 * Ensure required test directories exist and are empty
 */
pub fn setup() {
    let my_id = unsafe { NEXT_ID.fetch_add(1, atomics::SeqCst) };
    task_id.replace(Some(my_id));
    debug!("path setup; root={}; home={}", root().display(), home().display());
    root().rm_rf().unwrap();
    home().mkdir_p().unwrap();
}
