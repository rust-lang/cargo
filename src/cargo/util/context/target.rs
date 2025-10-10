use super::{CV, ConfigKey, ConfigRelativePath, GlobalContext, OptValue, PathAndArgs, StringList};
use crate::core::compiler::{BuildOutput, LibraryPath, LinkArgTarget};
use crate::util::CargoResult;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Config definition of a `[target.'cfg(…)']` table.
///
/// This is a subset of `TargetConfig`.
#[derive(Debug, Deserialize)]
pub struct TargetCfgConfig {
    pub runner: OptValue<PathAndArgs>,
    pub rustflags: OptValue<StringList>,
    pub linker: OptValue<ConfigRelativePath>,
    // This is here just to ignore fields from normal `TargetConfig` because
    // all `[target]` tables are getting deserialized, whether they start with
    // `cfg(` or not.
    #[serde(flatten)]
    pub other: BTreeMap<String, toml::Value>,
}

/// Config definition of a `[target]` table or `[host]`.
#[derive(Debug, Clone, Default)]
pub struct TargetConfig {
    /// Process to run as a wrapper for `cargo run`, `test`, and `bench` commands.
    pub runner: OptValue<PathAndArgs>,
    /// Additional rustc flags to pass.
    pub rustflags: OptValue<StringList>,
    /// Additional rustdoc flags to pass.
    pub rustdocflags: OptValue<StringList>,
    /// The path of the linker for this target.
    pub linker: OptValue<ConfigRelativePath>,
    /// Build script override for the given library name.
    ///
    /// Any package with a `links` value for the given library name will skip
    /// running its build script and instead use the given output from the
    /// config file.
    pub links_overrides: Rc<BTreeMap<String, BuildOutput>>,
}

/// Loads all of the `target.'cfg()'` tables.
pub(super) fn load_target_cfgs(
    gctx: &GlobalContext,
) -> CargoResult<Vec<(String, TargetCfgConfig)>> {
    // Load all [target] tables, filter out the cfg() entries.
    let mut result = Vec::new();
    // Use a BTreeMap so the keys are sorted. This is important for
    // deterministic ordering of rustflags, which affects fingerprinting and
    // rebuilds. We may perhaps one day wish to ensure a deterministic
    // ordering via the order keys were defined in files perhaps.
    let target: BTreeMap<String, TargetCfgConfig> = gctx.get("target")?;
    tracing::debug!("Got all targets {:#?}", target);
    for (key, cfg) in target {
        if let Ok(platform) = key.parse::<cargo_platform::Platform>() {
            let mut warnings = Vec::new();
            platform.check_cfg_keywords(&mut warnings, &Path::new(".cargo/config.toml"));
            for w in warnings {
                gctx.shell().warn(w)?;
            }
        }
        if key.starts_with("cfg(") {
            // Unfortunately this is not able to display the location of the
            // unused key. Using config::Value<toml::Value> doesn't work. One
            // solution might be to create a special "Any" type, but I think
            // that will be quite difficult with the current design.
            for other_key in cfg.other.keys() {
                gctx.shell().warn(format!(
                    "unused key `{}` in [target] config table `{}`",
                    other_key, key
                ))?;
            }
            result.push((key, cfg));
        }
    }
    Ok(result)
}

/// Returns true if the `[target]` table should be applied to host targets.
pub(super) fn get_target_applies_to_host(gctx: &GlobalContext) -> CargoResult<bool> {
    if gctx.cli_unstable().target_applies_to_host {
        if let Ok(target_applies_to_host) = gctx.get::<bool>("target-applies-to-host") {
            Ok(target_applies_to_host)
        } else {
            Ok(!gctx.cli_unstable().host_config)
        }
    } else if gctx.cli_unstable().host_config {
        anyhow::bail!(
            "the -Zhost-config flag requires the -Ztarget-applies-to-host flag to be set"
        );
    } else {
        Ok(true)
    }
}

/// Loads a single `[host]` table for the given triple.
pub(super) fn load_host_triple(gctx: &GlobalContext, triple: &str) -> CargoResult<TargetConfig> {
    if gctx.cli_unstable().host_config {
        let host_triple_prefix = format!("host.{}", triple);
        let host_triple_key = ConfigKey::from_str(&host_triple_prefix);
        let host_prefix = match gctx.get_cv(&host_triple_key)? {
            Some(_) => host_triple_prefix,
            None => "host".to_string(),
        };
        load_config_table(gctx, &host_prefix)
    } else {
        Ok(TargetConfig::default())
    }
}

/// Loads a single `[target]` table for the given triple.
pub(super) fn load_target_triple(gctx: &GlobalContext, triple: &str) -> CargoResult<TargetConfig> {
    load_config_table(gctx, &format!("target.{}", triple))
}

/// Loads a single table for the given prefix.
fn load_config_table(gctx: &GlobalContext, prefix: &str) -> CargoResult<TargetConfig> {
    // This needs to get each field individually because it cannot fetch the
    // struct all at once due to `links_overrides`. Can't use `serde(flatten)`
    // because it causes serde to use `deserialize_map` which means the config
    // deserializer does not know which keys to deserialize, which means
    // environment variables would not work.
    let runner: OptValue<PathAndArgs> = gctx.get(&format!("{prefix}.runner"))?;
    let rustflags: OptValue<StringList> = gctx.get(&format!("{prefix}.rustflags"))?;
    let rustdocflags: OptValue<StringList> = gctx.get(&format!("{prefix}.rustdocflags"))?;
    let linker: OptValue<ConfigRelativePath> = gctx.get(&format!("{prefix}.linker"))?;
    // Links do not support environment variables.
    let target_key = ConfigKey::from_str(prefix);
    let links_overrides = match gctx.get_table(&target_key)? {
        Some(links) => parse_links_overrides(&target_key, links.val)?,
        None => BTreeMap::new(),
    };
    Ok(TargetConfig {
        runner,
        rustflags,
        rustdocflags,
        linker,
        links_overrides: Rc::new(links_overrides),
    })
}

fn parse_links_overrides(
    target_key: &ConfigKey,
    links: HashMap<String, CV>,
) -> CargoResult<BTreeMap<String, BuildOutput>> {
    let mut links_overrides = BTreeMap::new();

    for (lib_name, value) in links {
        // Skip these keys, it shares the namespace with `TargetConfig`.
        match lib_name.as_str() {
            // `ar` is a historical thing.
            "ar" | "linker" | "runner" | "rustflags" | "rustdocflags" => continue,
            _ => {}
        }
        let mut output = BuildOutput::default();
        let table = value.table(&format!("{}.{}", target_key, lib_name))?.0;
        // We require deterministic order of evaluation, so we must sort the pairs by key first.
        let mut pairs = Vec::new();
        for (k, value) in table {
            pairs.push((k, value));
        }
        pairs.sort_by_key(|p| p.0);
        for (key, value) in pairs {
            match key.as_str() {
                "rustc-flags" => {
                    let flags = value.string(key)?;
                    let whence = format!("target config `{}.{}` (in {})", target_key, key, flags.1);
                    let (paths, links) = BuildOutput::parse_rustc_flags(flags.0, &whence)?;
                    output
                        .library_paths
                        .extend(paths.into_iter().map(LibraryPath::External));
                    output.library_links.extend(links);
                }
                "rustc-link-lib" => {
                    let list = value.string_list(key)?;
                    output
                        .library_links
                        .extend(list.iter().map(|v| v.0.clone()));
                }
                "rustc-link-search" => {
                    let list = value.string_list(key)?;
                    output.library_paths.extend(
                        list.iter()
                            .map(|v| PathBuf::from(&v.0))
                            .map(LibraryPath::External),
                    );
                }
                "rustc-link-arg-cdylib" | "rustc-cdylib-link-arg" => {
                    let args = extra_link_args(LinkArgTarget::Cdylib, key, value)?;
                    output.linker_args.extend(args);
                }
                "rustc-link-arg-bins" => {
                    let args = extra_link_args(LinkArgTarget::Bin, key, value)?;
                    output.linker_args.extend(args);
                }
                "rustc-link-arg" => {
                    let args = extra_link_args(LinkArgTarget::All, key, value)?;
                    output.linker_args.extend(args);
                }
                "rustc-link-arg-tests" => {
                    let args = extra_link_args(LinkArgTarget::Test, key, value)?;
                    output.linker_args.extend(args);
                }
                "rustc-link-arg-benches" => {
                    let args = extra_link_args(LinkArgTarget::Bench, key, value)?;
                    output.linker_args.extend(args);
                }
                "rustc-link-arg-examples" => {
                    let args = extra_link_args(LinkArgTarget::Example, key, value)?;
                    output.linker_args.extend(args);
                }
                "rustc-cfg" => {
                    let list = value.string_list(key)?;
                    output.cfgs.extend(list.iter().map(|v| v.0.clone()));
                }
                "rustc-check-cfg" => {
                    let list = value.string_list(key)?;
                    output.check_cfgs.extend(list.iter().map(|v| v.0.clone()));
                }
                "rustc-env" => {
                    for (name, val) in value.table(key)?.0 {
                        let val = val.string(name)?.0;
                        output.env.push((name.clone(), val.to_string()));
                    }
                }
                "warning" | "rerun-if-changed" | "rerun-if-env-changed" => {
                    anyhow::bail!("`{}` is not supported in build script overrides", key);
                }
                _ => {
                    let val = value.string(key)?.0;
                    output.metadata.push((key.clone(), val.to_string()));
                }
            }
        }
        links_overrides.insert(lib_name, output);
    }
    Ok(links_overrides)
}

fn extra_link_args(
    link_type: LinkArgTarget,
    key: &str,
    value: &CV,
) -> CargoResult<Vec<(LinkArgTarget, String)>> {
    let args = value.string_list(key)?;
    Ok(args.into_iter().map(|v| (link_type.clone(), v.0)).collect())
}
