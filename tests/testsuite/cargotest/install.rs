use std::fmt;
use std::path::{Path, PathBuf};

use hamcrest::{existing_file, MatchResult, Matcher};

use cargotest::support::paths;

pub use self::InstalledExe as has_installed_exe;

pub fn cargo_home() -> PathBuf {
    paths::home().join(".cargo")
}

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
