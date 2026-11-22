//! Config file discovery, `include` handling, and `--config` parsing

use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::MutexGuard;

use anyhow::Context as _;
use anyhow::Result;
use anyhow::bail;
use cargo_util::collections::HashMap;
use cargo_util::collections::HashSet;
use cargo_util::is_glob_pattern;
use cargo_util::paths;
use cargo_util_terminal::Shell;
use cargo_util_terminal::report::Level;

use super::ConfigValue;
use super::DefinitionKind;
use super::schema::{DetailedIncludeConfig, IncludeConfig};
use super::{ConfigKey, ConfigReader, ConfigStore};

use ConfigValue as CV;

/// Discovers and loads config files from the filesystem.
#[derive(Clone, Copy)]
pub struct ConfigLoader<'a> {
    shell: &'a Mutex<Shell>,
    current_dir: &'a Path,
    cargo_home: &'a Path,
    search_stop_path: Option<&'a Path>,
}

impl<'a> ConfigLoader<'a> {
    /// Creates a config loader.
    ///
    /// * `current_dir` is what `--config` arguments and `include`s are relative to on the command line.
    /// * `cargo_home` is `$CARGO_HOME`, where the lowest-precedence config file live.
    pub fn new(shell: &'a Mutex<Shell>, current_dir: &'a Path, cargo_home: &'a Path) -> Self {
        Self {
            shell,
            current_dir,
            cargo_home,
            search_stop_path: None,
        }
    }

    /// Sets a path where ancestor config file discovery will stop.
    ///
    /// The given path is included.
    #[must_use]
    pub fn search_stop_path(mut self, path: Option<&'a Path>) -> Self {
        self.search_stop_path = path;
        self
    }

    /// Discovers config files from a path without merging config values.
    ///
    /// Returns one [`ConfigStore`] per config file, from lowest to highest precedence.
    pub fn load_layers(&self, path: &Path) -> Result<Vec<ConfigStore>> {
        let mut result = Vec::new();
        self.walk_tree(path, |path| {
            let mut layers = Vec::new();
            self.load_file_layers(
                path,
                &mut HashSet::default(),
                DefinitionKind::Path,
                &mut layers,
            )?;
            // `walk_tree` visits highest precedence first,
            // but result is lowest-first,
            // so prepend to ensure the order
            result.splice(..0, layers);
            Ok(())
        })
        .context("could not load Cargo configuration")?;
        Ok(result)
    }

    /// Loads a config file and its `include`s recursively without merging config values.
    fn load_file_layers(
        &self,
        path: &Path,
        seen: &mut HashSet<PathBuf>,
        definition_kind: DefinitionKind,
        output: &mut Vec<ConfigStore>,
    ) -> Result<()> {
        if !seen.insert(path.to_path_buf()) {
            bail!(
                "config `include` cycle detected with path `{}`",
                path.display()
            );
        }
        tracing::debug!(?path, ?definition_kind, "load config from file");
        let config = ConfigStore::read_layer(path, definition_kind)?;
        self.load_include_layers(&config, seen, definition_kind, output)?;
        output.push(config);
        Ok(())
    }

    /// Loads the `include`s of `config` recursively without merging config values.
    fn load_include_layers(
        &self,
        config: &ConfigStore,
        seen: &mut HashSet<PathBuf>,
        definition_kind: DefinitionKind,
        output: &mut Vec<ConfigStore>,
    ) -> Result<()> {
        for include in self.include_paths(config)? {
            let Some(abs_path) = include.resolve_path(self.current_dir) else {
                continue;
            };
            self.load_file_layers(&abs_path, seen, definition_kind, output)
                .with_context(|| {
                    format!(
                        "failed to load config include `{}` from `{}`",
                        include.path,
                        include.path.definition()
                    )
                })?;
        }
        Ok(())
    }

    /// Reads the `include` entries of `config`.
    fn include_paths(&self, config: &ConfigStore) -> Result<Vec<DetailedIncludeConfig>> {
        let entries: Vec<IncludeConfig> = ConfigReader::new(self.shell)
            .config(config)
            .get(ConfigKey::from_parts(["include"]))?;
        let includes: Vec<_> = entries
            .into_iter()
            .map(IncludeConfig::into_detailed)
            .collect();

        for include in &includes {
            let path = &include.path;
            if path.extension() != Some(OsStr::new("toml")) {
                bail!(
                    "expected a config include path ending with `.toml`, \
                     but found `{}` from `{}`",
                    path,
                    path.definition(),
                )
            }

            if is_glob_pattern(path.as_str()) {
                bail!(
                    "expected a config include path without glob patterns, \
                     but found `{}` from `{}`",
                    path,
                    path.definition(),
                )
            }
            if path.as_str().contains(&['{', '}']) {
                bail!(
                    "expected a config include path without template braces, \
                     but found `{}` from `{}`",
                    path,
                    path.definition(),
                )
            }
        }

        Ok(includes)
    }

    /// Loads `--config` arguments and  merges them into one [`ConfigStore`].
    ///
    /// The `--config` value can be a path to a config file or a `key=value` pair.
    /// Later arguments take precedence over earlier ones.
    pub fn load_cli_args_layer(&self, cli_args: &[String]) -> Result<ConfigStore> {
        let mut loaded_args = ConfigStore::default();
        let mut seen = HashSet::default();
        for arg in cli_args {
            let mut layers = Vec::new();
            let arg_as_path = self.current_dir.join(arg);
            if !arg.is_empty() && arg_as_path.exists() {
                // --config path_to_file
                self.load_file_layers(&arg_as_path, &mut seen, DefinitionKind::Cli, &mut layers)
                    .with_context(|| {
                        format!("failed to load config from `{}`", arg_as_path.display())
                    })?;
            } else {
                let config = ConfigStore::parse_cli_key_value(arg)?;
                self.load_include_layers(
                    &config,
                    &mut HashSet::default(),
                    DefinitionKind::Cli,
                    &mut layers,
                )
                .context("failed to load --config include")?;
                layers.push(config);
            }
            loaded_args = ConfigStore::from_layers(std::iter::once(loaded_args).chain(layers))
                .with_context(|| format!("failed to merge --config argument `{arg}`"))?;
        }
        Ok(loaded_args)
    }

    /// Finds `<file_stem>` or `<file_stem>.toml` in `dir`.
    ///
    /// The file without extension is preferred if both exist.
    ///
    /// With `warn`, a warning is emitted if the legacy `<file_stem>` config file is used.
    pub fn get_file_path(
        &self,
        dir: &Path,
        file_stem: &str,
        warn: bool,
    ) -> Result<Option<PathBuf>> {
        let possible = dir.join(file_stem);
        let possible_with_extension = dir.join(format!("{}.toml", file_stem));

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
                                "`{}` is deprecated in favor of `{file_stem}.toml`",
                                possible.display(),
                            )).element(Level::HELP.message(
                            format!("if you need to support cargo 1.38 or earlier, you can symlink `{file_stem}` to `{file_stem}.toml`")))
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

    fn walk_tree<F>(&self, pwd: &Path, mut walk: F) -> Result<()>
    where
        F: FnMut(&Path) -> Result<()>,
    {
        let home = self.cargo_home;
        let mut seen_dir = HashSet::default();

        for current in paths::ancestors(pwd, self.search_stop_path) {
            let config_root = current.join(".cargo");
            if let Some(path) = self.get_file_path(&config_root, "config", true)? {
                walk(&path)?;
            }

            let canonical_root = config_root.canonicalize().unwrap_or(config_root);
            seen_dir.insert(canonical_root);
        }

        let canonical_home = home.canonicalize().unwrap_or(home.to_path_buf());

        // Once we're done, also be sure to walk the $CARGO_HOME directory even if it's not
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
    pub fn load_credentials(&self) -> Result<ConfigStore> {
        let Some(credentials) = self.get_file_path(self.cargo_home, "credentials", true)? else {
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

    fn shell(&self) -> MutexGuard<'_, Shell> {
        self.shell.lock().unwrap()
    }
}
