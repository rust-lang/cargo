use crate::aliased_command;
use crate::command_prelude::*;

use cargo::drop_println;
use cargo::util::errors::CargoResult;
use cargo_util::paths::resolve_executable;
use flate2::read::GzDecoder;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Read;
use std::io::Write;
use std::path::Path;

const COMPRESSED_MAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/man.tgz"));

pub fn cli() -> Command {
    subcommand("help")
        .about("Displays help for a cargo command")
        .arg(
            Arg::new("COMMAND")
                .num_args(1..)
                .action(ArgAction::Append)
                .add(clap_complete::ArgValueCandidates::new(|| {
                    super::builtin()
                        .iter()
                        .map(|cmd| {
                            let name = cmd.get_name();
                            clap_complete::CompletionCandidate::new(name)
                                .help(cmd.get_about().cloned())
                                .hide(cmd.is_hide_set())
                        })
                        .collect()
                })),
        )
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let args_command = args
        .get_many::<String>("COMMAND")
        .map(|vals| vals.map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    if args_command.is_empty() {
        let _ = crate::cli::cli(gctx).print_help();
        return Ok(());
    }

    let subcommand = if args_command.len() == 1 {
        // Expand alias first
        let subcommand = args_command.first().unwrap();
        match aliased_command(gctx, subcommand).ok().flatten() {
            Some(argv) if argv.len() > 1 => {
                // If this alias is more than a simple subcommand pass-through, show the alias.
                let alias = argv.join(" ");
                drop_println!(gctx, "`{}` is aliased to `{}`", subcommand, alias);
                return Ok(());
            }
            // Otherwise, resolve the alias into its subcommand.
            Some(argv) => {
                // An alias with an empty argv can be created via `"empty-alias" = ""`.
                let first = argv.get(0).map(String::as_str).unwrap_or(subcommand);
                first.to_string()
            }
            None => subcommand.to_string(),
        }
    } else {
        match validate_builtin_command_path(&args_command) {
            PathValidation::Valid => {}
            PathValidation::UnknownCommand(invalid) => {
                let err = anyhow::format_err!(
                    "no such command: `{invalid}`\n\n\
                     help: view all installed commands with `cargo --list`",
                );
                return Err(err.into());
            }
            PathValidation::UnknownSubcommand {
                valid_prefix,
                invalid,
            } => {
                let err = anyhow::format_err!(
                    "no such subcommand: `{invalid}`\n\n\
                     help: view available subcommands with `cargo {valid_prefix} --help`",
                );
                return Err(err.into());
            }
        }

        args_command.join("-")
    };

    let builtins = all_builtin_commands();
    if let Some(subcommand) = builtins.get(&subcommand).cloned() {
        if try_help(&subcommand)? {
            return Ok(());
        }
        crate::execute_internal_subcommand(gctx, &[OsStr::new(&subcommand), OsStr::new("--help")])?;
    } else {
        // If not built-in, try giving `--help` to external command.
        crate::execute_external_subcommand(
            gctx,
            &subcommand,
            &[OsStr::new(&subcommand), OsStr::new("--help")],
        )?;
    }

    Ok(())
}

fn try_help(subcommand: &str) -> CargoResult<bool> {
    #[expect(
        clippy::disallowed_methods,
        reason = "testing only, no reason for config support"
    )]
    let force_help_text = std::env::var("__CARGO_TEST_FORCE_HELP_TXT").is_ok();

    if resolve_executable(Path::new("man")).is_ok() && !force_help_text {
        let Some(man) = extract_man(subcommand, "1") else {
            return Ok(false);
        };
        write_and_spawn(subcommand, &man, "man")?;
    } else {
        let Some(txt) = extract_man(subcommand, "txt") else {
            return Ok(false);
        };
        if force_help_text {
            drop(std::io::stdout().write_all(&txt));
        } else if resolve_executable(Path::new("less")).is_ok() {
            write_and_spawn(subcommand, &txt, "less")?;
        } else if resolve_executable(Path::new("more")).is_ok() {
            write_and_spawn(subcommand, &txt, "more")?;
        } else {
            drop(std::io::stdout().write_all(&txt));
        }
    }
    Ok(true)
}

/// Extracts the given man page from the compressed archive.
///
/// Returns None if the command wasn't found.
fn extract_man(subcommand: &str, extension: &str) -> Option<Vec<u8>> {
    let extract_name = OsString::from(format!("cargo-{}.{}", subcommand, extension));
    let gz = GzDecoder::new(COMPRESSED_MAN);
    let mut ar = tar::Archive::new(gz);
    // Unwraps should be safe here, since this is a static archive generated
    // by our build script. It should never be an invalid format!
    for entry in ar.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap();
        if path.file_name().unwrap() != extract_name {
            continue;
        }
        let mut result = Vec::new();
        entry.read_to_end(&mut result).unwrap();
        return Some(result);
    }
    None
}

/// Write the contents of a man page to disk and spawn the given command to
/// display it.
fn write_and_spawn(name: &str, contents: &[u8], command: &str) -> CargoResult<()> {
    let prefix = format!("cargo-{}.", name);
    let mut tmp = tempfile::Builder::new().prefix(&prefix).tempfile()?;
    let f = tmp.as_file_mut();
    f.write_all(contents)?;
    f.flush()?;
    let path = tmp.path();
    // Use a path relative to the temp directory so that it can work on
    // cygwin/msys systems which don't handle windows-style paths.
    let mut relative_name = std::ffi::OsString::from("./");
    relative_name.push(path.file_name().unwrap());
    let mut cmd = std::process::Command::new(command)
        .arg(relative_name)
        .current_dir(path.parent().unwrap())
        .spawn()?;
    drop(cmd.wait());
    Ok(())
}

/// Result of validating a multi-arg command path.
enum PathValidation<'a> {
    Valid,
    UnknownCommand(&'a str),
    UnknownSubcommand {
        valid_prefix: String,
        invalid: &'a str,
    },
}

/// Validates that multi-arg paths represent actual nested commands.
fn validate_builtin_command_path<'a>(parts: &[&'a str]) -> PathValidation<'a> {
    let Some((first, remainings)) = parts.split_first() else {
        return PathValidation::Valid;
    };

    let builtins = super::builtin();

    let Some(mut current) = builtins.iter().find(|cmd| cmd.get_name() == *first) else {
        return PathValidation::UnknownCommand(first);
    };

    let mut valid_prefix = first.to_string();

    for &part in remainings {
        let next = current
            .get_subcommands()
            .find(|cmd| cmd.get_name() == part || cmd.get_all_aliases().any(|a| a == part));
        let Some(next) = next else {
            return PathValidation::UnknownSubcommand {
                valid_prefix,
                invalid: part,
            };
        };
        valid_prefix.push(' ');
        valid_prefix.push_str(next.get_name());
        current = next;
    }

    PathValidation::Valid
}

/// Builds a map of all command names (including nested and aliases) to their man page name.
fn all_builtin_commands() -> HashMap<String, String> {
    fn walk(cmd: Command, prefix: Option<&String>, map: &mut HashMap<String, String>) {
        let name = cmd.get_name();
        let man_page_name = match prefix {
            Some(prefix) => format!("{prefix}-{name}"),
            None => name.to_string(),
        };

        for cmd in cmd.get_subcommands() {
            walk(cmd.clone(), Some(&man_page_name), map);
        }

        for alias in cmd.get_all_aliases() {
            let alias_key = match prefix {
                Some(prefix) => format!("{prefix}-{alias}"),
                None => alias.to_string(),
            };
            map.insert(alias_key, man_page_name.clone());
        }

        map.insert(man_page_name.clone(), man_page_name);
    }

    let mut map = HashMap::new();
    for cmd in super::builtin() {
        walk(cmd, None, &mut map);
    }

    map
}
