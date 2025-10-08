//! Implementation of `cargo config` subcommand.

use crate::util::context::{ConfigKey, ConfigValue as CV, Definition, GlobalContext};
use crate::util::errors::CargoResult;
use crate::{drop_eprintln, drop_println};
use anyhow::{Error, bail, format_err};
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
    pub const POSSIBLE_VALUES: [&'static str; 3] = ["toml", "json", "json-value"];
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

pub fn get(gctx: &GlobalContext, opts: &GetOptions<'_>) -> CargoResult<()> {
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
        let cv = gctx
            .get_cv_with_env(&key)?
            .ok_or_else(|| format_err!("config value `{}` is not set", key))?;
        match opts.format {
            ConfigFormat::Toml => print_toml(gctx, opts, &key, &cv),
            ConfigFormat::Json => print_json(gctx, &key, &cv, true),
            ConfigFormat::JsonValue => print_json(gctx, &key, &cv, false),
        }
        if let Some(env) = maybe_env(gctx, &key, &cv) {
            match opts.format {
                ConfigFormat::Toml => print_toml_env(gctx, &env),
                ConfigFormat::Json | ConfigFormat::JsonValue => print_json_env(gctx, &env),
            }
        }
    } else {
        match &opts.format {
            ConfigFormat::Toml => print_toml_unmerged(gctx, opts, &key)?,
            format => bail!(
                "the `{}` format does not support --merged=no, try the `toml` format instead",
                format
            ),
        }
    }
    Ok(())
}

/// Checks for environment variables that might be used.
fn maybe_env<'gctx>(
    gctx: &'gctx GlobalContext,
    key: &ConfigKey,
    cv: &CV,
) -> Option<Vec<(&'gctx str, &'gctx str)>> {
    // Only fetching a table is unable to load env values. Leaf entries should
    // work properly.
    match cv {
        CV::Table(_map, _def) => {}
        _ => return None,
    }
    let mut env: Vec<_> = gctx
        .env()
        .filter(|(env_key, _val)| env_key.starts_with(&format!("{}_", key.as_env_key())))
        .collect();
    env.sort_by_key(|x| x.0);
    if env.is_empty() { None } else { Some(env) }
}

fn print_toml(gctx: &GlobalContext, opts: &GetOptions<'_>, key: &ConfigKey, cv: &CV) {
    let origin = |def: &Definition| -> String {
        if !opts.show_origin {
            return "".to_string();
        }
        format!(" # {}", def)
    };

    fn cv_to_toml(cv: &CV) -> toml_edit::Value {
        match cv {
            CV::String(s, _) => toml_edit::Value::from(s.as_str()),
            CV::Integer(i, _) => toml_edit::Value::from(*i),
            CV::Boolean(b, _) => toml_edit::Value::from(*b),
            CV::List(l, _) => toml_edit::Value::from_iter(l.iter().map(cv_to_toml)),
            CV::Table(t, _) => toml_edit::Value::from_iter({
                let mut t: Vec<_> = t.iter().collect();
                t.sort_by_key(|t| t.0);
                t.into_iter().map(|(k, v)| (k, cv_to_toml(v)))
            }),
        }
    }

    match cv {
        CV::Boolean(val, def) => drop_println!(gctx, "{} = {}{}", key, val, origin(def)),
        CV::Integer(val, def) => drop_println!(gctx, "{} = {}{}", key, val, origin(def)),
        CV::String(val, def) => drop_println!(
            gctx,
            "{} = {}{}",
            key,
            toml_edit::Value::from(val),
            origin(def)
        ),
        CV::List(vals, _def) => {
            if opts.show_origin {
                drop_println!(gctx, "{} = [", key);
                for cv in vals {
                    let val = cv_to_toml(cv);
                    let def = cv.definition();
                    drop_println!(gctx, "    {val}, # {def}");
                }
                drop_println!(gctx, "]");
            } else {
                let vals: toml_edit::Array = vals.iter().map(cv_to_toml).collect();
                drop_println!(gctx, "{} = {}", key, vals);
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
                print_toml(gctx, opts, &subkey, val);
            }
        }
    }
}

fn print_toml_env(gctx: &GlobalContext, env: &[(&str, &str)]) {
    drop_println!(
        gctx,
        "# The following environment variables may affect the loaded values."
    );
    for (env_key, env_value) in env {
        let val = shell_escape::escape(Cow::Borrowed(env_value));
        drop_println!(gctx, "# {}={}", env_key, val);
    }
}

fn print_json_env(gctx: &GlobalContext, env: &[(&str, &str)]) {
    drop_eprintln!(
        gctx,
        "note: The following environment variables may affect the loaded values."
    );
    for (env_key, env_value) in env {
        let val = shell_escape::escape(Cow::Borrowed(env_value));
        drop_eprintln!(gctx, "{}={}", env_key, val);
    }
}

fn print_json(gctx: &GlobalContext, key: &ConfigKey, cv: &CV, include_key: bool) {
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
    drop_println!(gctx, "{}", serde_json::to_string(&json_value).unwrap());

    // Helper for recursively converting a CV to JSON.
    fn cv_to_json(cv: &CV) -> serde_json::Value {
        match cv {
            CV::Boolean(val, _def) => json!(val),
            CV::Integer(val, _def) => json!(val),
            CV::String(val, _def) => json!(val),
            CV::List(vals, _def) => {
                let jvals: Vec<_> = vals.iter().map(cv_to_json).collect();
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

fn print_toml_unmerged(
    gctx: &GlobalContext,
    opts: &GetOptions<'_>,
    key: &ConfigKey,
) -> CargoResult<()> {
    let print_table = |cv: &CV| {
        drop_println!(gctx, "# {}", cv.definition());
        print_toml(gctx, opts, &ConfigKey::new(), cv);
        drop_println!(gctx, "");
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

    let mut cli_args = gctx.cli_args_as_table()?;
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
    let mut env: Vec<_> = gctx
        .env()
        .filter(|(env_key, _val)| env_key.starts_with(key.as_env_key()))
        .collect();
    if !env.is_empty() {
        env.sort_by_key(|x| x.0);
        drop_println!(gctx, "# Environment variables");
        for (key, value) in env {
            // Displaying this in "shell" syntax instead of TOML, since that
            // somehow makes more sense to me.
            let val = shell_escape::escape(Cow::Borrowed(value));
            drop_println!(gctx, "# {}={}", key, val);
        }
        drop_println!(gctx, "");
    }

    let unmerged = gctx.load_values_unmerged()?;
    for mut cv in unmerged {
        if trim_cv(&mut cv, key)? {
            print_table(&cv);
        }
    }
    Ok(())
}
