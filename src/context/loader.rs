//! Config file discovery, `include` handling, and `--config` parsing

use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use anyhow::bail;
use cargo_util::collections::HashMap;
use cargo_util::collections::HashSet;
use cargo_util::paths;
use cargo_util_terminal::report::Level;

use super::ConfigStore;
use super::ConfigValue;
use super::Definition;
use super::DefinitionKind;
use super::GlobalContext;
use super::MergePriority;
use crate::util::errors::CargoResult;
use crate::util::restricted_names::is_glob_pattern;

use ConfigValue as CV;

impl GlobalContext {
    /// Like [`load_config`](GlobalContext::load_config) but without merging config values.
    ///
    /// This is primarily crafted for `cargo config` command.
    pub(crate) fn load_config_unmerged(&self) -> CargoResult<Vec<ConfigStore>> {
        let mut result = Vec::new();
        let mut seen = HashSet::default();
        let home = self.home_path.clone().into_path_unlocked();
        self.walk_tree(&self.cwd, &home, |path| {
            let mut config = self._load_file(path, &mut seen, false, DefinitionKind::Path)?;
            self.load_unmerged_include(&mut config, &mut seen, &mut result)?;
            result.push(config);
            Ok(())
        })
        .context("could not load Cargo configuration")?;
        Ok(result)
    }

    /// Like [`load_includes`](GlobalContext::load_includes) but without merging config values.
    ///
    /// This is primarily crafted for `cargo config` command.
    fn load_unmerged_include(
        &self,
        config: &mut ConfigStore,
        seen: &mut HashSet<PathBuf>,
        output: &mut Vec<ConfigStore>,
    ) -> CargoResult<()> {
        let includes = self.include_paths(config, false)?;
        for include in includes {
            let Some(abs_path) = include.resolve_path(&self.cwd) else {
                continue;
            };

            let mut config = self
                ._load_file(&abs_path, seen, false, DefinitionKind::Path)
                .with_context(|| {
                    format!(
                        "failed to load config include `{}` from `{}`",
                        include.path.display(),
                        include.def
                    )
                })?;
            self.load_unmerged_include(&mut config, seen, output)?;
            output.push(config);
        }
        Ok(())
    }

    /// Start a config file discovery from a path and merges all config values found.
    pub(crate) fn load_config_from(&self, path: &Path) -> CargoResult<ConfigStore> {
        let mut cfg = ConfigStore::default();
        let home = self.home_path.clone().into_path_unlocked();

        self.walk_tree(path, &home, |path| {
            let config = self.load_file(path)?;
            cfg.merge(config, MergePriority::Definition)
                .with_context(|| {
                    format!("failed to merge configuration at `{}`", path.display())
                })?;
            Ok(())
        })
        .context("could not load Cargo configuration")?;

        Ok(cfg)
    }

    /// Loads configuration from a path.
    ///
    /// This is used during config file discovery.
    fn load_file(&self, path: &Path) -> CargoResult<ConfigStore> {
        self._load_file(path, &mut HashSet::default(), true, DefinitionKind::Path)
    }

    /// Loads configuration from a path with options.
    ///
    /// This is the implementation of loading configuration from a path.
    ///
    /// * `includes` determines whether to load configs from [`ConfigInclude`].
    /// * `seen` is used to check for cyclic includes.
    /// * `definition_kind` tells why a config is being loaded.
    fn _load_file(
        &self,
        path: &Path,
        seen: &mut HashSet<PathBuf>,
        includes: bool,
        definition_kind: DefinitionKind,
    ) -> CargoResult<ConfigStore> {
        if !seen.insert(path.to_path_buf()) {
            bail!(
                "config `include` cycle detected with path `{}`",
                path.display()
            );
        }
        tracing::debug!(?path, ?definition_kind, includes, "load config from file");

        let config = ConfigStore::read_layer(path, definition_kind)?;

        if includes {
            self.load_includes(config, seen, definition_kind)
        } else {
            Ok(config)
        }
    }

    /// Load any `include` files listed in the given `config`.
    ///
    /// Returns `config` with the given include files merged into it.
    ///
    /// * `seen` is used to check for cyclic includes.
    /// * `definition_kind` tells why a config is being loaded.
    fn load_includes(
        &self,
        mut config: ConfigStore,
        seen: &mut HashSet<PathBuf>,
        definition_kind: DefinitionKind,
    ) -> CargoResult<ConfigStore> {
        // Get the list of files to load.
        let includes = self.include_paths(&mut config, true)?;

        // Accumulate all values here.
        let root = CV::Table(HashMap::default(), config.definition().clone());
        let mut root = ConfigStore::from_table_value(root).unwrap();
        for include in includes {
            let Some(abs_path) = include.resolve_path(&self.cwd) else {
                continue;
            };

            self._load_file(&abs_path, seen, true, definition_kind)
                .and_then(|include| root.merge(include, MergePriority::From))
                .with_context(|| {
                    format!(
                        "failed to load config include `{}` from `{}`",
                        include.path.display(),
                        include.def
                    )
                })?;
        }
        root.merge(config, MergePriority::From)?;
        Ok(root)
    }

    /// Converts the `include` config value to a list of absolute paths.
    fn include_paths(
        &self,
        config: &mut ConfigStore,
        remove: bool,
    ) -> CargoResult<Vec<ConfigInclude>> {
        let table = config.values_mut();
        let include = if remove {
            table.remove("include").map(Cow::Owned)
        } else {
            table.get("include").map(Cow::Borrowed)
        };
        let includes = match include.map(|c| c.into_owned()) {
            Some(CV::List(list, _def)) => list
                .into_iter()
                .enumerate()
                .map(|(idx, cv)| match cv {
                    CV::String(s, def) => Ok(ConfigInclude::new(s, def)),
                    CV::Table(mut table, def) => {
                        // Extract `include.path`
                        let s = match table.remove("path") {
                            Some(CV::String(s, _)) => s,
                            Some(other) => bail!(
                                "expected a string, but found {} at `include[{idx}].path` in `{def}`",
                                other.desc()
                            ),
                            None => bail!("missing field `path` at `include[{idx}]` in `{def}`"),
                        };

                        // Extract optional `include.optional` field
                        let optional = match table.remove("optional") {
                            Some(CV::Boolean(b, _)) => b,
                            Some(other) => bail!(
                                "expected a boolean, but found {} at `include[{idx}].optional` in `{def}`",
                                other.desc()
                            ),
                            None => false,
                        };

                        let mut include = ConfigInclude::new(s, def);
                        include.optional = optional;
                        Ok(include)
                    }
                    other => bail!(
                        "expected a string or table, but found {} at `include[{idx}]` in {}",
                        other.desc(),
                        other.definition(),
                    ),
                })
                .collect::<CargoResult<Vec<_>>>()?,
            Some(other) => bail!(
                "expected a list of strings or a list of tables, but found {} at `include` in `{}",
                other.desc(),
                other.definition()
            ),
            None => {
                return Ok(Vec::new());
            }
        };

        for include in &includes {
            if include.path.extension() != Some(OsStr::new("toml")) {
                bail!(
                    "expected a config include path ending with `.toml`, \
                     but found `{}` from `{}`",
                    include.path.display(),
                    include.def,
                )
            }

            if let Some(path) = include.path.to_str() {
                // Ignore non UTF-8 bytes as glob and template syntax are for textual config.
                if is_glob_pattern(path) {
                    bail!(
                        "expected a config include path without glob patterns, \
                         but found `{}` from `{}`",
                        include.path.display(),
                        include.def,
                    )
                }
                if path.contains(&['{', '}']) {
                    bail!(
                        "expected a config include path without template braces, \
                         but found `{}` from `{}`",
                        include.path.display(),
                        include.def,
                    )
                }
            }
        }

        Ok(includes)
    }

    /// Parses the CLI config args and returns them as a table.
    pub(crate) fn cli_args_to_config(&self) -> CargoResult<ConfigStore> {
        let mut loaded_args =
            ConfigStore::from_table_value(CV::Table(HashMap::default(), Definition::Cli(None)))
                .unwrap();
        let Some(cli_args) = &self.cli_config else {
            return Ok(loaded_args);
        };
        let mut seen = HashSet::default();
        for arg in cli_args {
            let arg_as_path = self.cwd.join(arg);
            let arg_config = if !arg.is_empty() && arg_as_path.exists() {
                // --config path_to_file
                self._load_file(&arg_as_path, &mut seen, true, DefinitionKind::Cli)
                    .with_context(|| {
                        format!("failed to load config from `{}`", arg_as_path.display())
                    })?
            } else {
                ConfigStore::parse_cli_key_value(arg)?
            };
            let arg_config = self
                .load_includes(arg_config, &mut HashSet::default(), DefinitionKind::Cli)
                .context("failed to load --config include".to_string())?;
            loaded_args
                .merge(arg_config, MergePriority::From)
                .with_context(|| format!("failed to merge --config argument `{arg}`"))?;
        }
        Ok(loaded_args)
    }

    /// The purpose of this function is to aid in the transition to using
    /// .toml extensions on Cargo's config files, which were historically not used.
    /// Both 'config.toml' and 'credentials.toml' should be valid with or without extension.
    /// When both exist, we want to prefer the one without an extension for
    /// backwards compatibility, but warn the user appropriately.
    pub(crate) fn get_file_path(
        &self,
        dir: &Path,
        filename_without_extension: &str,
        warn: bool,
    ) -> CargoResult<Option<PathBuf>> {
        let possible = dir.join(filename_without_extension);
        let possible_with_extension = dir.join(format!("{}.toml", filename_without_extension));

        if let Ok(possible_handle) = same_file::Handle::from_path(&possible) {
            if warn {
                if let Ok(possible_with_extension_handle) =
                    same_file::Handle::from_path(&possible_with_extension)
                {
                    // We don't want to print a warning if the version
                    // without the extension is just a symlink to the version
                    // WITH an extension, which people may want to do to
                    // support multiple Cargo versions at once and not
                    // get a warning.
                    if possible_handle != possible_with_extension_handle {
                        self.shell().warn(format!(
                            "both `{}` and `{}` exist. Using `{}`",
                            possible.display(),
                            possible_with_extension.display(),
                            possible.display()
                        ))?;
                    }
                } else {
                    self.shell().print_report(&[
                        Level::WARNING.secondary_title(
                            format!(
                                "`{}` is deprecated in favor of `{filename_without_extension}.toml`",
                                possible.display(),
                            )).element(Level::HELP.message(
                            format!("if you need to support cargo 1.38 or earlier, you can symlink `{filename_without_extension}` to `{filename_without_extension}.toml`")))
                    ], false)?;
                }
            }

            Ok(Some(possible))
        } else if possible_with_extension.exists() {
            Ok(Some(possible_with_extension))
        } else {
            Ok(None)
        }
    }

    fn walk_tree<F>(&self, pwd: &Path, home: &Path, mut walk: F) -> CargoResult<()>
    where
        F: FnMut(&Path) -> CargoResult<()>,
    {
        let mut seen_dir = HashSet::default();

        for current in paths::ancestors(pwd, self.search_stop_path.as_deref()) {
            let config_root = current.join(".cargo");
            if let Some(path) = self.get_file_path(&config_root, "config", true)? {
                walk(&path)?;
            }

            let canonical_root = config_root.canonicalize().unwrap_or(config_root);
            seen_dir.insert(canonical_root);
        }

        let canonical_home = home.canonicalize().unwrap_or(home.to_path_buf());

        // Once we're done, also be sure to walk the home directory even if it's not
        // in our history to be sure we pick up that standard location for
        // information.
        if !seen_dir.contains(&canonical_home) && !seen_dir.contains(home) {
            if let Some(path) = self.get_file_path(home, "config", true)? {
                walk(&path)?;
            }
        }

        Ok(())
    }

    /// Loads credentials config from the credentials file, if present.
    pub(crate) fn load_credentials(&self) -> CargoResult<ConfigStore> {
        let home_path = self.home_path.clone().into_path_unlocked();
        let Some(credentials) = self.get_file_path(&home_path, "credentials", true)? else {
            return Ok(ConfigStore::default());
        };

        let mut config = ConfigStore::read_layer(&credentials, DefinitionKind::Path)?;
        // Backwards compatibility for old `.cargo/credentials` layout.
        {
            let def = config.definition().clone();
            let value_map = config.values_mut();

            if let Some(token) = value_map.remove("token") {
                value_map.entry("registry".into()).or_insert_with(|| {
                    let map = HashMap::from_iter([("token".into(), token)]);
                    CV::Table(map, def.clone())
                });
            }
        }

        Ok(config)
    }
}

/// Represents a config-include value in the configuration.
///
/// This intentionally doesn't derive serde deserialization
/// to avoid any misuse of `GlobalContext::get::<ConfigInclude>()`,
/// which might lead to wrong config loading order.
struct ConfigInclude {
    /// Path to a config-include configuration file.
    /// Could be either relative or absolute.
    path: PathBuf,
    def: Definition,
    /// Whether this include is optional (missing files are silently ignored)
    optional: bool,
}

impl ConfigInclude {
    fn new(p: impl Into<PathBuf>, def: Definition) -> Self {
        Self {
            path: p.into(),
            def,
            optional: false,
        }
    }

    /// Resolves the absolute path for this include.
    ///
    /// For file based include,
    /// it is relative to parent directory of the config file includes it.
    /// For example, if `.cargo/config.toml has a `include = "foo.toml"`,
    /// Cargo will load `.cargo/foo.toml`.
    ///
    /// For CLI based include (e.g., `--config 'include = "foo.toml"'`),
    /// it is relative to the current working directory.
    ///
    /// Returns `None` if this is an optional include and the file doesn't exist.
    /// Otherwise returns `Some(PathBuf)` with the absolute path.
    fn resolve_path(&self, cwd: &Path) -> Option<PathBuf> {
        let abs_path = match &self.def {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.parent().unwrap(),
            Definition::Environment(_) | Definition::Cli(None) | Definition::BuiltIn => cwd,
        }
        .join(&self.path);
        let abs_path = paths::normalize_path(&abs_path);

        if self.optional && !abs_path.exists() {
            tracing::info!(
                "skipping optional include `{}` in `{}`:  file not found at `{}`",
                self.path.display(),
                self.def,
                abs_path.display(),
            );
            None
        } else {
            Some(abs_path)
        }
    }
}
