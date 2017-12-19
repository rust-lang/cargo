use std::io::prelude::*;
use std::io;

use cargo::ops;
use cargo::core::{SourceId, Source};
use cargo::sources::RegistrySource;
use cargo::util::{CargoError, CliResult, CargoResultExt, Config};

#[derive(Deserialize)]
pub struct Options {
    flag_host: Option<String>,
    arg_token: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
    flag_registry: Option<String>,
}

pub const USAGE: &'static str = "
Save an api token from the registry locally. If token is not specified, it will be read from stdin.

Usage:
    cargo login [options] [<token>]

Options:
    -h, --help               Print this message
    --host HOST              Host to set the token for
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo
    --registry REGISTRY      Registry to use

";

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;

    if options.flag_registry.is_some() && !config.cli_unstable().unstable_options {
        return Err(CargoError::from("registry option is an unstable feature and requires -Zunstable-options to use.").into());
    }

    let token = match options.arg_token {
        Some(token) => token,
        None        => {
            let host = match options.flag_host.clone() {
                Some(host)  => host,
                None        => {
                    // If the host flag wasn't set, check if the registry supports the login
                    // interface.
                    use cargo::util::ToUrl;

                    // Get the registry source ID
                    let sid = match options.flag_registry {
                        Some(ref registry)  => {
                            let ops::RegistryConfig {
                                index, ..
                            } = ops::registry_configuration(config, Some(registry.clone()))?;
                            match index {
                                Some(index) => SourceId::for_registry(&index.to_url()?)?,
                                None        => {
                                    let err_msg = format!("registry `{}` not configured", registry);
                                    return Err(CargoError::from(err_msg).into())
                                }
                            }
                        }
                        None            => SourceId::crates_io(config)?,
                    };

                    // Update the registry and access its configuration
                    let mut src = RegistrySource::remote(&sid, config);
                    src.update().chain_err(|| format!("failed to update {}", sid))?;
                    let src_cfg = src.config()?.unwrap();

                    // If the registry supports the v1 login flow, you can use its
                    // api root as the host.
                    if src_cfg.commands.get("login").map_or(false, |vs| vs.iter().any(|v| v == "v1")) {
                        src_cfg.api.unwrap()
                    } else {
                        return Err(CargoError::from("token must be provided when --registry is provided.").into());
                    }
                }
            };

            println!("please visit {}me and paste the API Token below", host);
            let mut line = String::new();
            let input = io::stdin();
            input.lock().read_line(&mut line).chain_err(|| {
                "failed to read stdin"
            })?;
            line.trim().to_string()
        }
    };

    ops::registry_login(config, token, options.flag_registry)?;
    Ok(())
}
