use std::time::Duration;

pub use self::canonical_url::CanonicalUrl;
pub use self::config::{homedir, Config, ConfigValue};
pub use self::dependency_queue::DependencyQueue;
pub use self::diagnostic_server::RustfixDiagnosticServer;
pub use self::errors::{internal, process_error};
pub use self::errors::{CargoResult, CargoResultExt, CliResult, Test};
pub use self::errors::{CargoTestError, CliError, ProcessError};
pub use self::flock::{FileLock, Filesystem};
pub use self::graph::Graph;
pub use self::hex::{hash_u64, short_hash, to_hex};
pub use self::into_url::IntoUrl;
pub use self::into_url_with_base::IntoUrlWithBase;
pub use self::lev_distance::{closest, closest_msg, lev_distance};
pub use self::lockserver::{LockServer, LockServerClient, LockServerStarted};
pub use self::paths::{bytes2path, dylib_path, join_paths, path2bytes};
pub use self::paths::{dylib_path_envvar, normalize_path};
pub use self::process_builder::{process, ProcessBuilder};
pub use self::progress::{Progress, ProgressStyle};
pub use self::read2::read2;
pub use self::rustc::Rustc;
pub use self::sha256::Sha256;
pub use self::to_semver::ToSemver;
pub use self::vcs::{existing_vcs_repo, FossilRepo, GitRepo, HgRepo, PijulRepo};
pub use self::workspace::{
    print_available_benches, print_available_binaries, print_available_examples,
    print_available_tests,
};

mod canonical_url;
pub mod command_prelude;
pub mod config;
pub mod cpu;
mod dependency_queue;
pub mod diagnostic_server;
pub mod errors;
mod flock;
pub mod graph;
pub mod hex;
pub mod important_paths;
pub mod into_url;
mod into_url_with_base;
pub mod job;
pub mod lev_distance;
mod lockserver;
pub mod machine_message;
pub mod network;
pub mod paths;
pub mod process_builder;
pub mod profile;
mod progress;
mod read2;
pub mod rustc;
mod sha256;
pub mod to_semver;
pub mod toml;
mod vcs;
mod workspace;

pub fn elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}.{:02}s", secs, duration.subsec_nanos() / 10_000_000)
    }
}

/// Check the base requirements for a package name.
///
/// This can be used for other things than package names, to enforce some
/// level of sanity. Note that package names have other restrictions
/// elsewhere. `cargo new` has a few restrictions, such as checking for
/// reserved names. crates.io has even more restrictions.
pub fn validate_package_name(name: &str, what: &str, help: &str) -> CargoResult<()> {
    if let Some(ch) = name
        .chars()
        .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
    {
        anyhow::bail!("Invalid character `{}` in {}: `{}`{}", ch, what, name, help);
    }
    Ok(())
}

/// Whether or not this running in a Continuous Integration environment.
pub fn is_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("TF_BUILD").is_ok()
}
