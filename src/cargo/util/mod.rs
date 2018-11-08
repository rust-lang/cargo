use std::time::Duration;

pub use self::cfg::{Cfg, CfgExpr, Platform};
pub use self::config::{homedir, Config, ConfigValue};
pub use self::dependency_queue::{DependencyQueue, Dirty, Fresh, Freshness};
pub use self::errors::{CargoError, CargoResult, CargoResultExt, CliResult, Test};
pub use self::errors::{CargoTestError, CliError, ProcessError};
pub use self::errors::{internal, process_error};
pub use self::flock::{FileLock, Filesystem};
pub use self::graph::Graph;
pub use self::hex::{short_hash, to_hex, hash_u64};
pub use self::lev_distance::lev_distance;
pub use self::paths::{dylib_path, join_paths, bytes2path, path2bytes};
pub use self::paths::{dylib_path_envvar, normalize_path, without_prefix};
pub use self::process_builder::{process, ProcessBuilder};
pub use self::rustc::Rustc;
pub use self::sha256::Sha256;
pub use self::to_semver::ToSemver;
pub use self::to_url::ToUrl;
pub use self::vcs::{FossilRepo, GitRepo, HgRepo, PijulRepo, existing_vcs_repo};
pub use self::read2::read2;
pub use self::progress::{Progress, ProgressStyle};
pub use self::lockserver::{LockServer, LockServerStarted, LockServerClient};
pub use self::diagnostic_server::RustfixDiagnosticServer;

pub mod config;
pub mod errors;
pub mod graph;
pub mod hex;
pub mod important_paths;
pub mod job;
pub mod lev_distance;
pub mod machine_message;
pub mod network;
pub mod paths;
pub mod process_builder;
pub mod profile;
pub mod to_semver;
pub mod to_url;
pub mod toml;
mod cfg;
mod dependency_queue;
mod rustc;
mod sha256;
mod vcs;
mod flock;
mod read2;
mod progress;
mod lockserver;
pub mod diagnostic_server;

pub fn elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}.{:02}s", secs, duration.subsec_nanos() / 10_000_000)
    }
}
