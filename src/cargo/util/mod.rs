use std::time::Duration;

pub use self::cfg::{Cfg, CfgExpr};
pub use self::config::{homedir, Config, ConfigValue};
pub use self::dependency_queue::{DependencyQueue, Dirty, Fresh, Freshness};
pub use self::diagnostic_server::RustfixDiagnosticServer;
pub use self::errors::{internal, process_error};
pub use self::errors::{CargoResult, CargoResultExt, CliResult, Test};
pub use self::errors::{CargoTestError, CliError, ProcessError};
pub use self::flock::{FileLock, Filesystem};
pub use self::graph::Graph;
pub use self::hex::{hash_u64, short_hash, to_hex};
pub use self::lev_distance::lev_distance;
pub use self::lockserver::{LockServer, LockServerClient, LockServerStarted};
pub use self::paths::{bytes2path, dylib_path, join_paths, path2bytes};
pub use self::paths::{dylib_path_envvar, normalize_path, without_prefix};
pub use self::process_builder::{process, ProcessBuilder};
pub use self::progress::{Progress, ProgressStyle};
pub use self::read2::read2;
pub use self::rustc::Rustc;
pub use self::sha256::Sha256;
pub use self::to_semver::ToSemver;
pub use self::to_url::ToUrl;
pub use self::vcs::{existing_vcs_repo, FossilRepo, GitRepo, HgRepo, PijulRepo};

mod cfg;
pub mod command_prelude;
pub mod config;
mod dependency_queue;
pub mod diagnostic_server;
pub mod errors;
mod flock;
pub mod graph;
pub mod hex;
pub mod important_paths;
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
mod rustc;
mod sha256;
pub mod to_semver;
pub mod to_url;
pub mod toml;
mod vcs;

pub fn elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}.{:02}s", secs, duration.subsec_nanos() / 10_000_000)
    }
}
