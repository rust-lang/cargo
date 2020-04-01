//! An abstraction over what directories cargo should use for state

use crate::util::{
    config::Filesystem,
    errors::{CargoResult, CargoResultExt},
};
use directories::ProjectDirs;
use log::debug;
use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct CargoDirs {
    /// Main directory for cargo data
    pub data_dir: Filesystem,
    /// Caching registry artefacts (previously .cargo/registry/cache)
    pub cache_dir: Filesystem,
    /// Kept to walk upwards the directory tree to find a Cargo.toml
    pub home_dir: Filesystem,
}

impl CargoDirs {
    /// Constructs the hierarchy of directories that cargo will use
    pub fn new(home_dir: PathBuf) -> CargoResult<CargoDirs> {
        let current_dir =
            env::current_dir().chain_err(|| "couldn't get the current directory of the process")?;

        let mut cache_dir = PathBuf::default();
        let mut data_dir = PathBuf::default();

        // 1. CARGO_HOME set
        let cargo_home_env = env::var_os("CARGO_HOME").map(|home| current_dir.join(home));
        if let Some(cargo_home) = cargo_home_env.clone() {
            cache_dir = cargo_home.clone();
            data_dir = cargo_home.clone();
        }

        // 2. CARGO_CACHE_DIR, CARGO_CONFIG_DIR, CARGO_BIN_DIR, ... set
        let cargo_cache_env = env::var_os("CARGO_CACHE_DIR").map(|home| current_dir.join(home));
        let cargo_data_env = env::var_os("CARGO_DATA_DIR").map(|home| current_dir.join(home));

        if let Some(cargo_cache) = cargo_cache_env.clone() {
            cache_dir = cargo_cache.clone();
        }
        if let Some(cargo_data) = cargo_data_env.clone() {
            data_dir = cargo_data.clone();
        }

        // none of the env vars are set ...
        if cargo_home_env.is_none() && cargo_cache_env.is_none() && cargo_data_env.is_none() {
            let legacy_cargo_dir = home_dir.join(".cargo");

            // 3. ... and .cargo exist
            if legacy_cargo_dir.exists() {
                debug!("Using legacy paths at $HOME, consider moving to $XDG_DATA_HOME");
                cache_dir = legacy_cargo_dir.clone();
                data_dir = legacy_cargo_dir.clone();

            // 4. ... otherwise follow platform conventions
            } else {
                let xdg_dirs = match ProjectDirs::from("org", "rust-lang", "cargo") {
                    Some(d) => Ok(d),
                    None => Err(anyhow::format_err!(
                        "failed to get directories according to XDG settings"
                    )),
                }?;

                cache_dir = xdg_dirs.cache_dir().to_path_buf();
                data_dir = xdg_dirs.data_dir().to_path_buf();
            }
        }

        dbg!(Ok(CargoDirs {
            cache_dir: Filesystem::new(cache_dir),
            data_dir: Filesystem::new(data_dir),
            home_dir: Filesystem::new(home_dir),
        }))
    }
}
