//! Implementation of `cargo config` subcommand.

use crate::util::config::{Config, ConfigKey, ConfigValue as CV, Definition};
use crate::util::errors::CargoResult;
use crate::{drop_eprintln, drop_println};
use anyhow::{bail, format_err, Error};
use serde_json::json;
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

pub enum ConfigFormat {
    Toml,
    Json,
    JsonValue,
}

impl ConfigFormat {
    /// For clap.
    pub const POSSIBLE_VALUES: &'static [&'static str] = &["toml", "json", "json-value"];
}

impl FromStr for ConfigFormat {
    type Err = Error;
    fn from_str(s: &str) -> CargoResult<Self> {
        match s {
            "toml" => Ok(ConfigFormat::Toml),
            "json" => Ok(ConfigFormat::Json),
            "json-value" => Ok(ConfigFormat::JsonValue),
            f => bail!("unknown config format `{}`", f),
        }
    }
}

impl fmt::Display for ConfigFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ConfigFormat::Toml => write!(f, "toml"),
            ConfigFormat::Json => write!(f, "json"),
            ConfigFormat::JsonValue => write!(f, "json-value"),
        }
    }
}

/// Options for `cargo config get`.
pub struct GetOptions<'a> {
    pub key: Option<&'a str>,
    pub format: ConfigFormat,
    pub show_origin: bool,
    pub merged: bool,
}

pub fn get(config: &Config, opts: &GetOptions<'_>) -> CargoResult<()> {
    if opts.show_origin && !matches!(opts.format, ConfigFormat::Toml) {
        bail!(
            "the `{}` format does not support --show-origin, try the `toml` format instead",
            opts.format
        );
    }
    let key = match opts.key {
        Some(key) => ConfigKey::from_str(key),
        None => ConfigKey::new(),
    };
    if opts.merged {
        let cv = config
            .get_cv_with_env(&key)?
            .ok_or_else(|| format_err!("config value `{}` is not set", key))?;
        match opts.format {
            ConfigFormat::Toml => print_toml(config, opts, &key, &cv),
            ConfigFormat::Json => print_json(config, &key, &cv, true),
            ConfigFormat::JsonValue => print_json(config, &key, &cv, false),
        }
        if let Some(env) = maybe_env(config, &key, &cv) {
            match opts.format {
                ConfigFormat::Toml => print_toml_env(config, &env),
                ConfigFormat::Json | ConfigFormat::JsonValue => print_json_env(config, &env),
            }
        }
    } else {
        match &opts.format {
            ConfigFormat::Toml => print_toml_unmerged(config, opts, &key)?,
            format => bail!(
                "the `{}` format does not support --merged=no, try the `toml` format instead",
                format
            ),
        }
    }
    Ok(())
}

/// Checks for environment variables that might be used.
fn maybe_env<'config>(
    config: &'config Config,
    key: &ConfigKey,
    cv: &CV,
) -> Option<Vec<(&'config String, &'config String)>> {
    // Only fetching a table is unable to load env values. Leaf entries should
    // work properly.
    match cv {
        CV::Table(_map, _def) => {}
        _ => return None,
    }
    let mut env: Vec<_> = config
        .env()
        .iter()
        .filter(|(env_key, _val)| env_key.starts_with(&format!("{}_", key.as_env_key())))
        .collect();
    env.sort_by_key(|x| x.0);
    if env.is_empty() {
        None
    } else {
        Some(env)
    }
}

fn print_toml(config: &Config, opts: &GetOptions<'_>, key: &ConfigKey, cv: &CV) {
    let origin = |def: &Definition| -> String {
        if !opts.show_origin {
            return "".to_string();
        }
        format!(" # {}", def)
    };
    match cv {
        CV::Boolean(val, def) => drop_println!(config, "{} = {}{}", key, val, origin(def)),
        CV::Integer(val, def) => drop_println!(config, "{} = {}{}", key, val, origin(def)),
        CV::String(val, def) => drop_println!(
            config,
            "{} = {}{}",
            key,
            toml::to_string(&val).unwrap(),
            origin(def)
        ),
        CV::List(vals, _def) => {
            if opts.show_origin {
                drop_println!(config, "{} = [", key);
                for (val, def) in vals {
                    drop_println!(config, "    {}, # {}", toml::to_string(&val).unwrap(), def);
                }
                drop_println!(config, "]");
            } else {
                let vals: Vec<&String> = vals.iter().map(|x| &x.0).collect();
                drop_println!(config, "{} = {}", key, toml::to_string(&vals).unwrap());
            }
        }
        CV::Table(table, _def) => {
            let mut key_vals: Vec<_> = table.iter().collect();
            key_vals.sort_by(|a, b| a.0.cmp(b.0));
            for (table_key, val) in key_vals {
                let mut subkey = key.clone();
                // push or push_sensitive shouldn't matter here, since this is
                // not dealing with environment variables.
                subkey.push(table_key);
                print_toml(config, opts, &subkey, val);
            }
        }
    }
}

fn print_toml_env(config: &Config, env: &[(&String, &String)]) {
    drop_println!(
        config,
        "# The following environment variables may affect the loaded values."
    );
    for (env_key, env_value) in env {
        let val = shell_escape::escape(Cow::Borrowed(env_value));
        drop_println!(config, "# {}={}", env_key, val);
    }
}

fn print_json_env(config: &Config, env: &[(&String, &String)]) {
    drop_eprintln!(
        config,
        "note: The following environment variables may affect the loaded values."
    );
    for (env_key, env_value) in env {
        let val = shell_escape::escape(Cow::Borrowed(env_value));
        drop_eprintln!(config, "{}={}", env_key, val);
    }
}

fn print_json(config: &Config, key: &ConfigKey, cv: &CV, include_key: bool) {
    let json_value = if key.is_root() || !include_key {
        cv_to_json(cv)
    } else {
        let mut parts: Vec<_> = key.parts().collect();
        let last_part = parts.pop().unwrap();
        let mut root_table = json!({});
        // Create a JSON object with nested keys up to the value being displayed.
        let mut table = &mut root_table;
        for part in parts {
            table[part] = json!({});
            table = table.get_mut(part).unwrap();
        }
        table[last_part] = cv_to_json(cv);
        root_table
    };
    drop_println!(config, "{}", serde_json::to_string(&json_value).unwrap());

    // Helper for recursively converting a CV to JSON.
    fn cv_to_json(cv: &CV) -> serde_json::Value {
        match cv {
            CV::Boolean(val, _def) => json!(val),
            CV::Integer(val, _def) => json!(val),
            CV::String(val, _def) => json!(val),
            CV::List(vals, _def) => {
                let jvals: Vec<_> = vals.iter().map(|(val, _def)| json!(val)).collect();
                json!(jvals)
            }
            CV::Table(map, _def) => {
                let mut table = json!({});
                for (key, val) in map {
                    table[key] = cv_to_json(val);
                }
                table
            }
        }
    }
}

fn print_toml_unmerged(config: &Config, opts: &GetOptions<'_>, key: &ConfigKey) -> CargoResult<()> {
    let print_table = |cv: &CV| {
        drop_println!(config, "# {}", cv.definition());
        print_toml(config, opts, &ConfigKey::new(), cv);
        drop_println!(config, "");
    };
    // This removes entries from the given CV so that all that remains is the
    // given key. Returns false if no entries were found.
    fn trim_cv(mut cv: &mut CV, key: &ConfigKey) -> CargoResult<bool> {
        for (i, part) in key.parts().enumerate() {
            match cv {
                CV::Table(map, _def) => {
                    map.retain(|key, _value| key == part);
                    match map.get_mut(part) {
                        Some(val) => cv = val,
                        None => return Ok(false),
                    }
                }
                _ => {
                    let mut key_so_far = ConfigKey::new();
                    for part in key.parts().take(i) {
                        key_so_far.push(part);
                    }
                    bail!(
                        "expected table for configuration key `{}`, \
                         but found {} in {}",
                        key_so_far,
                        cv.desc(),
                        cv.definition()
                    )
                }
            }
        }
        Ok(match cv {
            CV::Table(map, _def) => !map.is_empty(),
            _ => true,
        })
    }

    let mut cli_args = config.cli_args_as_table()?;
    if trim_cv(&mut cli_args, key)? {
        print_table(&cli_args);
    }

    // This slurps up some extra env vars that aren't technically part of the
    // "config" (or are special-cased). I'm personally fine with just keeping
    // them here, though it might be confusing. The vars I'm aware of:
    //
    // * CARGO
    // * CARGO_HOME
    // * CARGO_NAME
    // * CARGO_EMAIL
    // * CARGO_INCREMENTAL
    // * CARGO_TARGET_DIR
    // * CARGO_CACHE_RUSTC_INFO
    //
    // All of these except CARGO, CARGO_HOME, and CARGO_CACHE_RUSTC_INFO are
    // actually part of the config, but they are special-cased in the code.
    //
    // TODO: It might be a good idea to teach the Config loader to support
    // environment variable aliases so that these special cases are less
    // special, and will just naturally get loaded as part of the config.
    let mut env: Vec<_> = config
        .env()
        .iter()
        .filter(|(env_key, _val)| env_key.starts_with(key.as_env_key()))
        .collect();
    if !env.is_empty() {
        env.sort_by_key(|x| x.0);
        drop_println!(config, "# Environment variables");
        for (key, value) in env {
            // Displaying this in "shell" syntax instead of TOML, since that
            // somehow makes more sense to me.
            let val = shell_escape::escape(Cow::Borrowed(value));
            drop_println!(config, "# {}={}", key, val);
        }
        drop_println!(config, "");
    }

    let unmerged = config.load_values_unmerged()?;
    for mut cv in unmerged {
        if trim_cv(&mut cv, key)? {
            print_table(&cv);
        }
    }
    Ok(())
}
