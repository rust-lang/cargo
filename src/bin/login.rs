use std::io::prelude::*;
use std::io;

use cargo::ops;
use cargo::core::{SourceId, Source};
use cargo::sources::RegistrySource;
use cargo::util::{CliResult, CargoResultExt, Config};

#[derive(Deserialize)]
pub struct Options {
    flag_host: Option<String>, // TODO: Depricated, remove
    flag_index: Option<String>,
    arg_token: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Save an api token from the registry locally

Usage:
    cargo login [options] [<token>]

Options:
    -h, --help               Print this message
    --index INDEX            Registry index to search in
    --host HOST              DEPRECATED, renamed to '--index'
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo

";

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;

    // TODO: Depricated
    // remove once it has been decided --host can be safely removed
    // We may instead want to repurpose the host flag, as
    // mentioned in this issue
    // https://github.com/rust-lang/cargo/issues/4208

    let msg = "The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index in which to search. Please
use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.";

    let index = match options.flag_host {
        Some(host) => {
            if !host.is_empty() {
                config.shell().warn(&msg)?;
                Some(host)
            } else {
                options.flag_index
            }
        },
        None => options.flag_index
    };

    let token = match options.arg_token {
        Some(token) => token,
        None => {
            let index = match index {
                Some(index) => index,
                None => {
                    let src = SourceId::crates_io(config)?;
                    let mut src = RegistrySource::remote(&src, config);
                    src.update()?;
                    src.config()?.unwrap().api
                }
            };

            println!("please visit {}me and paste the API Token below", index);
            let mut line = String::new();
            let input = io::stdin();
            input.lock().read_line(&mut line).chain_err(|| {
                "failed to read stdin"
            })?;
            line.trim().to_string()
        }
    };

    ops::registry_login(config, token)?;
    Ok(())
}
