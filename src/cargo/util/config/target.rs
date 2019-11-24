use super::{Config, ConfigRelativePath, OptValue, PathAndArgs, StringList};
use crate::core::compiler::BuildOutput;
use crate::util::CargoResult;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Config definition of a [target.'cfg(…)'] table.
///
/// This is a subset of `TargetConfig`.
#[derive(Debug, Deserialize)]
pub struct TargetCfgConfig {
    pub runner: OptValue<PathAndArgs>,
    pub rustflags: OptValue<StringList>,
    // This is here just to ignore fields from normal `TargetConfig` because
    // all `[target]` tables are getting deserialized, whether they start with
    // `cfg(` or not.
    #[serde(flatten)]
    pub other: BTreeMap<String, toml::Value>,
}

/// Config definition of a [target] table.
#[derive(Debug, Deserialize)]
pub struct TargetConfig {
    /// Process to run as a wrapper for `cargo run`, `test`, and `bench` commands.
    pub runner: OptValue<PathAndArgs>,
    /// Additional rustc flags to pass.
    pub rustflags: OptValue<StringList>,
    /// The path of the linker for this target.
    pub linker: OptValue<ConfigRelativePath>,
    /// The path of archiver (lib builder) for this target.
    pub ar: OptValue<ConfigRelativePath>,
    /// Build script override for the given library name.
    ///
    /// Any package with a `links` value for the given library name will skip
    /// running its build script and instead use the given output from the
    /// config file.
    #[serde(flatten)]
    pub links_overrides: BTreeMap<String, BuildOutput>,
}

pub(super) fn load_target_cfgs(config: &Config) -> CargoResult<Vec<(String, TargetCfgConfig)>> {
    // Load all [target] tables, filter out the cfg() entries.
    let mut result = Vec::new();
    // Use a BTreeMap so the keys are sorted. This is important for
    // deterministic ordering of rustflags, which affects fingerprinting and
    // rebuilds. We may perhaps one day wish to ensure a deterministic
    // ordering via the order keys were defined in files perhaps.
    log::debug!("Loading all targets.");
    let target: BTreeMap<String, TargetCfgConfig> = config.get("target")?;
    log::debug!("Got all targets {:#?}", target);
    for (key, cfg) in target {
        if key.starts_with("cfg(") {
            // Unfortunately this is not able to display the location of the
            // unused key. Using config::Value<toml::Value> doesn't work. One
            // solution might be to create a special "Any" type, but I think
            // that will be quite difficult with the current design.
            for other_key in cfg.other.keys() {
                config.shell().warn(format!(
                    "unused key `{}` in [target] config table `{}`",
                    other_key, key
                ))?;
            }
            result.push((key, cfg));
        }
    }
    Ok(result)
}
