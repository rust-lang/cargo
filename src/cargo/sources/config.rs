//! Implementation of configuration for various sources
//!
//! This module will parse the various `source.*` TOML configuration keys into a
//! structure usable by Cargo itself. Currently this is primarily used to map
//! sources to one another via the `replace-with` key in `.cargo/config`.

use crate::core::{GitReference, PackageId, Source, SourceId};
use crate::sources::{ReplacedSource, CRATES_IO_REGISTRY};
use crate::util::config::{self, ConfigRelativePath, OptValue};
use crate::util::errors::CargoResult;
use crate::util::{Config, IntoUrl};
use anyhow::{bail, Context as _};
use log::debug;
use std::collections::{HashMap, HashSet};
use url::Url;

#[derive(Clone)]
pub struct SourceConfigMap<'cfg> {
    /// Mapping of source name to the toml configuration.
    cfgs: HashMap<String, SourceConfig>,
    /// Mapping of `SourceId` to the source name.
    id2name: HashMap<SourceId, String>,
    config: &'cfg Config,
}

/// Definition of a source in a config file.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SourceConfigDef {
    /// Indicates this source should be replaced with another of the given name.
    replace_with: OptValue<String>,
    /// A directory source.
    directory: Option<ConfigRelativePath>,
    /// A registry source. Value is a URL.
    registry: OptValue<String>,
    /// A local registry source.
    local_registry: Option<ConfigRelativePath>,
    /// A git source. Value is a URL.
    git: OptValue<String>,
    /// The git branch.
    branch: OptValue<String>,
    /// The git tag.
    tag: OptValue<String>,
    /// The git revision.
    rev: OptValue<String>,
}

/// Configuration for a particular source, found in TOML looking like:
///
/// ```toml
/// [source.crates-io]
/// registry = 'https://github.com/rust-lang/crates.io-index'
/// replace-with = 'foo'    # optional
/// ```
#[derive(Clone)]
struct SourceConfig {
    /// `SourceId` this source corresponds to, inferred from the various
    /// defined keys in the configuration.
    id: SourceId,

    /// Whether or not this source is replaced with another.
    ///
    /// This field is a tuple of `(name, location)` where `location` is where
    /// this configuration key was defined (such as the `.cargo/config` path
    /// or the environment variable name).
    replace_with: Option<(String, String)>,
}

impl<'cfg> SourceConfigMap<'cfg> {
    pub fn new(config: &'cfg Config) -> CargoResult<SourceConfigMap<'cfg>> {
        let mut base = SourceConfigMap::empty(config)?;
        let sources: Option<HashMap<String, SourceConfigDef>> = config.get("source")?;
        if let Some(sources) = sources {
            for (key, value) in sources.into_iter() {
                base.add_config(key, value)?;
            }
        }
        Ok(base)
    }

    pub fn empty(config: &'cfg Config) -> CargoResult<SourceConfigMap<'cfg>> {
        let mut base = SourceConfigMap {
            cfgs: HashMap::new(),
            id2name: HashMap::new(),
            config,
        };
        base.add(
            CRATES_IO_REGISTRY,
            SourceConfig {
                id: SourceId::crates_io(config)?,
                replace_with: None,
            },
        )?;
        Ok(base)
    }

    pub fn config(&self) -> &'cfg Config {
        self.config
    }

    /// Get the `Source` for a given `SourceId`.
    pub fn load(
        &self,
        id: SourceId,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn Source + 'cfg>> {
        debug!("loading: {}", id);

        let mut name = match self.id2name.get(&id) {
            Some(name) => name,
            None => return id.load(self.config, yanked_whitelist),
        };
        let mut cfg_loc = "";
        let orig_name = name;
        let new_id;
        loop {
            let cfg = match self.cfgs.get(name) {
                Some(cfg) => cfg,
                None => bail!(
                    "could not find a configured source with the \
                     name `{}` when attempting to lookup `{}` \
                     (configuration in `{}`)",
                    name,
                    orig_name,
                    cfg_loc
                ),
            };
            match &cfg.replace_with {
                Some((s, c)) => {
                    name = s;
                    cfg_loc = c;
                }
                None if id == cfg.id => return id.load(self.config, yanked_whitelist),
                None => {
                    new_id = cfg.id.with_precise(id.precise().map(|s| s.to_string()));
                    break;
                }
            }
            debug!("following pointer to {}", name);
            if name == orig_name {
                bail!(
                    "detected a cycle of `replace-with` sources, the source \
                     `{}` is eventually replaced with itself \
                     (configuration in `{}`)",
                    name,
                    cfg_loc
                )
            }
        }

        let new_src = new_id.load(
            self.config,
            &yanked_whitelist
                .iter()
                .map(|p| p.map_source(id, new_id))
                .collect(),
        )?;
        let old_src = id.load(self.config, yanked_whitelist)?;
        if !new_src.supports_checksums() && old_src.supports_checksums() {
            bail!(
                "\
cannot replace `{orig}` with `{name}`, the source `{orig}` supports \
checksums, but `{name}` does not

a lock file compatible with `{orig}` cannot be generated in this situation
",
                orig = orig_name,
                name = name
            );
        }

        if old_src.requires_precise() && id.precise().is_none() {
            bail!(
                "\
the source {orig} requires a lock file to be present first before it can be
used against vendored source code

remove the source replacement configuration, generate a lock file, and then
restore the source replacement configuration to continue the build
",
                orig = orig_name
            );
        }

        Ok(Box::new(ReplacedSource::new(id, new_id, new_src)))
    }

    fn add(&mut self, name: &str, cfg: SourceConfig) -> CargoResult<()> {
        if let Some(old_name) = self.id2name.insert(cfg.id, name.to_string()) {
            // The user is allowed to redefine the built-in crates-io
            // definition from `empty()`.
            if name != CRATES_IO_REGISTRY {
                bail!(
                    "source `{}` defines source {}, but that source is already defined by `{}`\n\
                     note: Sources are not allowed to be defined multiple times.",
                    name,
                    cfg.id,
                    old_name
                );
            }
        }
        self.cfgs.insert(name.to_string(), cfg);
        Ok(())
    }

    fn add_config(&mut self, name: String, def: SourceConfigDef) -> CargoResult<()> {
        let mut srcs = Vec::new();
        if let Some(registry) = def.registry {
            let url = url(&registry, &format!("source.{}.registry", name))?;
            srcs.push(SourceId::for_alt_registry(&url, &name)?);
        }
        if let Some(local_registry) = def.local_registry {
            let path = local_registry.resolve_path(self.config);
            srcs.push(SourceId::for_local_registry(&path)?);
        }
        if let Some(directory) = def.directory {
            let path = directory.resolve_path(self.config);
            srcs.push(SourceId::for_directory(&path)?);
        }
        if let Some(git) = def.git {
            let url = url(&git, &format!("source.{}.git", name))?;
            let reference = match def.branch {
                Some(b) => GitReference::Branch(b.val),
                None => match def.tag {
                    Some(b) => GitReference::Tag(b.val),
                    None => match def.rev {
                        Some(b) => GitReference::Rev(b.val),
                        None => GitReference::DefaultBranch,
                    },
                },
            };
            srcs.push(SourceId::for_git(&url, reference)?);
        } else {
            let check_not_set = |key, v: OptValue<String>| {
                if let Some(val) = v {
                    bail!(
                        "source definition `source.{}` specifies `{}`, \
                         but that requires a `git` key to be specified (in {})",
                        name,
                        key,
                        val.definition
                    );
                }
                Ok(())
            };
            check_not_set("branch", def.branch)?;
            check_not_set("tag", def.tag)?;
            check_not_set("rev", def.rev)?;
        }
        if name == CRATES_IO_REGISTRY && srcs.is_empty() {
            srcs.push(SourceId::crates_io(self.config)?);
        }

        match srcs.len() {
            0 => bail!(
                "no source location specified for `source.{}`, need \
                 `registry`, `local-registry`, `directory`, or `git` defined",
                name
            ),
            1 => {}
            _ => bail!(
                "more than one source location specified for `source.{}`",
                name
            ),
        }
        let src = srcs[0];

        let replace_with = def
            .replace_with
            .map(|val| (val.val, val.definition.to_string()));

        self.add(
            &name,
            SourceConfig {
                id: src,
                replace_with,
            },
        )?;

        return Ok(());

        fn url(val: &config::Value<String>, key: &str) -> CargoResult<Url> {
            let url = val.val.into_url().with_context(|| {
                format!(
                    "configuration key `{}` specified an invalid \
                     URL (in {})",
                    key, val.definition
                )
            })?;

            Ok(url)
        }
    }
}
