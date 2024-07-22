//! Helpers for testing `cargo install`

use std::env::consts::EXE_SUFFIX;
use std::path::Path;

/// Used by `cargo install` tests to assert an executable binary
/// has been installed. Example usage:
/// ```no_run
/// use cargo_test_support::install::assert_has_installed_exe;
/// use cargo_test_support::paths;
///
/// assert_has_installed_exe(paths::cargo_home(), "foo");
/// ```
#[track_caller]
pub fn assert_has_installed_exe<P: AsRef<Path>>(path: P, name: &'static str) {
    assert!(check_has_installed_exe(path, name));
}

#[track_caller]
pub fn assert_has_not_installed_exe<P: AsRef<Path>>(path: P, name: &'static str) {
    assert!(!check_has_installed_exe(path, name));
}

fn check_has_installed_exe<P: AsRef<Path>>(path: P, name: &'static str) -> bool {
    path.as_ref().join("bin").join(exe(name)).is_file()
}

/// `$name$EXE`
pub fn exe(name: &str) -> String {
    format!("{}{}", name, EXE_SUFFIX)
}
