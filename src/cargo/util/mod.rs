use std::path::{Path, PathBuf};
use std::time::Duration;

pub use self::canonical_url::CanonicalUrl;
pub use self::context::{homedir, ConfigValue, GlobalContext};
pub(crate) use self::counter::MetricsCounter;
pub use self::dependency_queue::DependencyQueue;
pub use self::diagnostic_server::RustfixDiagnosticServer;
pub use self::edit_distance::{closest, closest_msg, edit_distance};
pub use self::errors::CliError;
pub use self::errors::{internal, CargoResult, CliResult};
pub use self::flock::{FileLock, Filesystem};
pub use self::graph::Graph;
pub use self::hasher::StableHasher;
pub use self::hex::{hash_u64, short_hash, to_hex};
pub use self::hostname::hostname;
pub use self::into_url::IntoUrl;
pub use self::into_url_with_base::IntoUrlWithBase;
pub(crate) use self::io::LimitErrorReader;
pub use self::lockserver::{LockServer, LockServerClient, LockServerStarted};
pub use self::progress::{Progress, ProgressStyle};
pub use self::queue::Queue;
pub use self::rustc::Rustc;
pub use self::semver_ext::{OptVersionReq, VersionExt};
pub use self::vcs::{existing_vcs_repo, FossilRepo, GitRepo, HgRepo, PijulRepo};
pub use self::workspace::{
    add_path_args, path_args, print_available_benches, print_available_binaries,
    print_available_examples, print_available_packages, print_available_tests,
};

pub mod auth;
pub mod cache_lock;
mod canonical_url;
pub mod command_prelude;
pub mod context;
mod counter;
pub mod cpu;
pub mod credential;
mod dependency_queue;
pub mod diagnostic_server;
pub mod edit_distance;
pub mod errors;
mod flock;
pub mod graph;
mod hasher;
pub mod hex;
mod hostname;
pub mod important_paths;
pub mod interning;
pub mod into_url;
mod into_url_with_base;
mod io;
pub mod job;
pub mod lints;
mod lockserver;
pub mod machine_message;
pub mod network;
mod progress;
mod queue;
pub mod restricted_names;
pub mod rustc;
mod semver_eval_ext;
mod semver_ext;
pub mod sqlite;
pub mod style;
pub mod toml;
pub mod toml_mut;
mod vcs;
mod workspace;

pub fn is_rustup() -> bool {
    // ALLOWED: `RUSTUP_HOME` should only be read from process env, otherwise
    // other tools may point to executables from incompatible distributions.
    #[allow(clippy::disallowed_methods)]
    std::env::var_os("RUSTUP_HOME").is_some()
}

pub fn elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}.{:02}s", secs, duration.subsec_nanos() / 10_000_000)
    }
}

/// Formats a number of bytes into a human readable SI-prefixed size.
/// Returns a tuple of `(quantity, units)`.
pub fn human_readable_bytes(bytes: u64) -> (f32, &'static str) {
    static UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let bytes = bytes as f32;
    let i = ((bytes.log2() / 10.0) as usize).min(UNITS.len() - 1);
    (bytes / 1024_f32.powi(i as i32), UNITS[i])
}

pub fn indented_lines(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::from("\n")
            } else {
                format!("  {}\n", line)
            }
        })
        .collect()
}

pub fn truncate_with_ellipsis(s: &str, max_width: usize) -> String {
    // We should truncate at grapheme-boundary and compute character-widths,
    // yet the dependencies on unicode-segmentation and unicode-width are
    // not worth it.
    let mut chars = s.chars();
    let mut prefix = (&mut chars).take(max_width - 1).collect::<String>();
    if chars.next().is_some() {
        prefix.push('…');
    }
    prefix
}

#[cfg(not(windows))]
#[inline]
pub fn try_canonicalize<P: AsRef<Path>>(path: P) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(&path)
}

#[cfg(windows)]
#[inline]
pub fn try_canonicalize<P: AsRef<Path>>(path: P) -> std::io::Result<PathBuf> {
    use std::io::Error;
    use std::io::ErrorKind;

    // On Windows `canonicalize` may fail, so we fall back to getting an absolute path.
    std::fs::canonicalize(&path).or_else(|_| {
        // Return an error if a file does not exist for better compatibility with `canonicalize`
        if !path.as_ref().try_exists()? {
            return Err(Error::new(ErrorKind::NotFound, "the path was not found"));
        }
        std::path::absolute(&path)
    })
}

/// Get the current [`umask`] value.
///
/// [`umask`]: https://man7.org/linux/man-pages/man2/umask.2.html
#[cfg(unix)]
pub fn get_umask() -> u32 {
    use std::sync::OnceLock;
    static UMASK: OnceLock<libc::mode_t> = OnceLock::new();
    // SAFETY: Syscalls are unsafe. Calling `umask` twice is even unsafer for
    // multithreading program, since it doesn't provide a way to retrieve the
    // value without modifications. We use a static `OnceLock` here to ensure
    // it only gets call once during the entire program lifetime.
    *UMASK.get_or_init(|| unsafe {
        let umask = libc::umask(0o022);
        libc::umask(umask);
        umask
    }) as u32 // it is u16 on macos
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_human_readable_bytes() {
        assert_eq!(human_readable_bytes(0), (0., "B"));
        assert_eq!(human_readable_bytes(8), (8., "B"));
        assert_eq!(human_readable_bytes(1000), (1000., "B"));
        assert_eq!(human_readable_bytes(1024), (1., "KiB"));
        assert_eq!(human_readable_bytes(1024 * 420 + 512), (420.5, "KiB"));
        assert_eq!(human_readable_bytes(1024 * 1024), (1., "MiB"));
        assert_eq!(
            human_readable_bytes(1024 * 1024 + 1024 * 256),
            (1.25, "MiB")
        );
        assert_eq!(human_readable_bytes(1024 * 1024 * 1024), (1., "GiB"));
        assert_eq!(
            human_readable_bytes((1024. * 1024. * 1024. * 3.1415) as u64),
            (3.1415, "GiB")
        );
        assert_eq!(human_readable_bytes(1024 * 1024 * 1024 * 1024), (1., "TiB"));
        assert_eq!(
            human_readable_bytes(1024 * 1024 * 1024 * 1024 * 1024),
            (1., "PiB")
        );
        assert_eq!(
            human_readable_bytes(1024 * 1024 * 1024 * 1024 * 1024 * 1024),
            (1., "EiB")
        );
        assert_eq!(human_readable_bytes(u64::MAX), (16., "EiB"));
    }
}
