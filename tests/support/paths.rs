use std::{io,os};
use std::io::IoResult;
use std::io::fs;
use cargo::util::realpath;

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cargo-integration-tests";

pub fn root() -> Path {
    realpath(&os::tmpdir().join(CARGO_INTEGRATION_TEST_DIR)).unwrap()
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
    debug!("path setup; root={}; home={}", root().display(), home().display());
    root().rm_rf().unwrap();
    home().mkdir_p().unwrap();
}
