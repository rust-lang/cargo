//! Implementation of configuration for various sources.
//!
//! This module will parse the various `source.*` TOML configuration keys into a
//! structure usable by Cargo itself. Currently this is primarily used to map
//! sources to one another via the `replace-with` key in `.cargo/config`.

use crate::core::{GitReference, PackageId, SourceId};
use crate::sources::overlay::DependencyConfusionThreatOverlaySource;
use crate::sources::source::Source;
use crate::sources::{ReplacedSource, CRATES_IO_REGISTRY};
use crate::util::context::{self, ConfigRelativePath, OptValue};
use crate::util::errors::CargoResult;
use crate::util::{GlobalContext, IntoUrl};
use anyhow::{bail, Context as _};
use std::collections::{HashMap, HashSet};
use tracing::debug;
use url::Url;

/// Represents the entire [`[source]` replacement table][1] in Cargo configuration.
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/config.html#source
#[derive(Clone)]
pub struct SourceConfigMap<'gctx> {
    /// Mapping of source name to the toml configuration.
    cfgs: HashMap<String, SourceConfig>,
    /// Mapping of [`SourceId`] to the source name.
    id2name: HashMap<SourceId, String>,
    /// Mapping of sources to local registries that will be overlaid on them.
    overlays: HashMap<SourceId, SourceId>,
    gctx: &'gctx GlobalContext,
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

impl<'gctx> SourceConfigMap<'gctx> {
    /// Like [`SourceConfigMap::empty`] but includes sources from source
    /// replacement configurations.
    pub fn new(gctx: &'gctx GlobalContext) -> CargoResult<SourceConfigMap<'gctx>> {
        let mut base = SourceConfigMap::empty(gctx)?;
        let sources: Option<HashMap<String, SourceConfigDef>> = gctx.get("source")?;
        if let Some(sources) = sources {
            for (key, value) in sources.into_iter() {
                base.add_config(key, value)?;
            }
        }

        Ok(base)
    }

    /// Like [`SourceConfigMap::new`] but includes sources from source
    /// replacement configurations.
    pub fn new_with_overlays(
        gctx: &'gctx GlobalContext,
        overlays: impl IntoIterator<Item = (SourceId, SourceId)>,
    ) -> CargoResult<SourceConfigMap<'gctx>> {
        let mut base = SourceConfigMap::new(gctx)?;
        base.overlays = overlays.into_iter().collect();
        Ok(base)
    }

    /// Creates the default set of sources that doesn't take `[source]`
    /// replacement into account.
    pub fn empty(gctx: &'gctx GlobalContext) -> CargoResult<SourceConfigMap<'gctx>> {
        let mut base = SourceConfigMap {
            cfgs: HashMap::new(),
            id2name: HashMap::new(),
            overlays: HashMap::new(),
            gctx,
        };
        base.add(
            CRATES_IO_REGISTRY,
            SourceConfig {
                id: SourceId::crates_io(gctx)?,
                replace_with: None,
            },
        )?;
        if SourceId::crates_io_is_sparse(gctx)? {
            base.add(
                CRATES_IO_REGISTRY,
                SourceConfig {
                    id: SourceId::crates_io_maybe_sparse_http(gctx)?,
                    replace_with: None,
                },
            )?;
        }
        if let Ok(url) = gctx.get_env("__CARGO_TEST_CRATES_IO_URL_DO_NOT_USE_THIS") {
            base.add(
                CRATES_IO_REGISTRY,
                SourceConfig {
                    id: SourceId::for_alt_registry(&url.parse()?, CRATES_IO_REGISTRY)?,
                    replace_with: None,
                },
            )?;
        }
        Ok(base)
    }

    /// Returns the [`GlobalContext`] this source config map is associated with.
    pub fn gctx(&self) -> &'gctx GlobalContext {
        self.gctx
    }

    /// Gets the [`Source`] for a given [`SourceId`].
    ///
    /// * `yanked_whitelist` --- Packages allowed to be used, even if they are yanked.
    pub fn load(
        &self,
        id: SourceId,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn Source + 'gctx>> {
        debug!("loading: {}", id);

        let Some(mut name) = self.id2name.get(&id) else {
            return self.load_overlaid(id, yanked_whitelist);
        };
        let mut cfg_loc = "";
        let orig_name = name;
        let new_id = loop {
            let Some(cfg) = self.cfgs.get(name) else {
                // Attempt to interpret the source name as an alt registry name
                if let Ok(alt_id) = SourceId::alt_registry(self.gctx, name) {
                    debug!("following pointer to registry {}", name);
                    break alt_id.with_precise_from(id);
                }
                bail!(
                    "could not find a configured source with the \
                     name `{}` when attempting to lookup `{}` \
                     (configuration in `{}`)",
                    name,
                    orig_name,
                    cfg_loc
                );
            };
            match &cfg.replace_with {
                Some((s, c)) => {
                    name = s;
                    cfg_loc = c;
                }
                None if id == cfg.id => return self.load_overlaid(id, yanked_whitelist),
                None => {
                    break cfg.id.with_precise_from(id);
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
        };

        let new_src = self.load_overlaid(
            new_id,
            &yanked_whitelist
                .iter()
                .map(|p| p.map_source(id, new_id))
                .collect(),
        )?;
        let old_src = id.load(self.gctx, yanked_whitelist)?;
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

        if old_src.requires_precise() && !id.has_precise() {
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

    /// Gets the [`Source`] for a given [`SourceId`] without performing any source replacement.
    fn load_overlaid(
        &self,
        id: SourceId,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn Source + 'gctx>> {
        let src = id.load(self.gctx, yanked_whitelist)?;
        if let Some(overlay_id) = self.overlays.get(&id) {
            let overlay = overlay_id.load(self.gctx(), yanked_whitelist)?;
            Ok(Box::new(DependencyConfusionThreatOverlaySource::new(
                overlay, src,
            )))
        } else {
            Ok(src)
        }
    }

    /// Adds a source config with an associated name.
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

    /// Adds a source config from TOML definition.
    fn add_config(&mut self, name: String, def: SourceConfigDef) -> CargoResult<()> {
        let mut srcs = Vec::new();
        if let Some(registry) = def.registry {
            let url = url(&registry, &format!("source.{}.registry", name))?;
            srcs.push(SourceId::for_source_replacement_registry(&url, &name)?);
        }
        if let Some(local_registry) = def.local_registry {
            let path = local_registry.resolve_path(self.gctx);
            srcs.push(SourceId::for_local_registry(&path)?);
        }
        if let Some(directory) = def.directory {
            let path = directory.resolve_path(self.gctx);
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
            srcs.push(SourceId::crates_io_maybe_sparse_http(self.gctx)?);
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

        fn url(val: &context::Value<String>, key: &str) -> CargoResult<Url> {
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
