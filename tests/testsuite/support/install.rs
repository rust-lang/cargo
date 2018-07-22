use std::fmt;
use std::path::{Path, PathBuf};

use support::hamcrest::{existing_file, MatchResult, Matcher};

use support::paths;

pub use self::InstalledExe as has_installed_exe;

pub fn cargo_home() -> PathBuf {
    paths::home().join(".cargo")
}

/// A `Matcher` used by `cargo install` tests to check if an executable binary
/// has been installed.  Example usage:
///
///     assert_that(cargo_home(), has_installed_exe("foo"));
pub struct InstalledExe(pub &'static str);

pub fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

impl<P: AsRef<Path>> Matcher<P> for InstalledExe {
    fn matches(&self, path: P) -> MatchResult {
        let path = path.as_ref().join("bin").join(exe(self.0));
        existing_file().matches(&path)
    }
}

impl fmt::Debug for InstalledExe {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "installed exe `{}`", self.0)
    }
}
