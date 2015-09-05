use std::path::{Path};

use util::config::{Config};
use util::{CargoResult, internal, ChainError};
use core::{SourceId};

/// Read the `paths` configuration variable to discover all path overrides that
/// have been configured.
pub fn source_ids_from_config(config: &Config, cur_path: &Path)
                          -> CargoResult<Vec<SourceId>> {

    let configs = try!(config.values());
    debug!("loaded config; configs={:?}", configs);
    let config_paths = match configs.get("paths") {
        Some(cfg) => cfg,
        None => return Ok(Vec::new())
    };
    let paths = try!(config_paths.list().chain_error(|| {
        internal("invalid configuration for the key `paths`")
    }));

    paths.iter().map(|&(ref s, ref p)| {
        // The path listed next to the string is the config file in which the
        // key was located, so we want to pop off the `.cargo/config` component
        // to get the directory containing the `.cargo` folder.
        p.parent().unwrap().parent().unwrap().join(s)
    }).filter(|p| {
        // Make sure we don't override the local package, even if it's in the
        // list of override paths.
        cur_path != &**p
    }).map(|p| SourceId::for_path(&p)).collect()
}
