use crate::aliased_command;
use crate::command_prelude::*;

use cargo::drop_println;
use cargo::util::errors::CargoResult;
use cargo_util::paths::resolve_executable;
use flate2::read::GzDecoder;

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

    let cmd: String;
    let lookup_parts: Vec<&str> = if args_command.len() == 1 {
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
                cmd = argv
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| subcommand.to_string());
                vec![cmd.as_str()]
            }
            None => args_command.clone(),
        }
    } else {
        args_command.clone()
    };

    match find_builtin_cmd(&lookup_parts) {
        Ok(path) => {
            let man_page_name = path.join("-");
            if try_help(&man_page_name)? {
                return Ok(());
            }
            crate::execute_internal_subcommand(
                gctx,
                &[OsStr::new(&man_page_name), OsStr::new("--help")],
            )?;
        }
        Err(FindError::UnknownCommand(cmd)) => {
            if lookup_parts.len() == 1 {
                if let Some(man_page_name) = find_builtin_cmd_dash_joined(cmd) {
                    if try_help(&man_page_name)? {
                        return Ok(());
                    }
                    crate::execute_internal_subcommand(
                        gctx,
                        &[OsStr::new(&man_page_name), OsStr::new("--help")],
                    )?;
                } else {
                    crate::execute_external_subcommand(
                        gctx,
                        cmd,
                        &[OsStr::new(cmd), OsStr::new("--help")],
                    )?;
                }
            } else {
                let err = anyhow::format_err!(
                    "no such command: `{cmd}`\n\n\
                     help: view all installed commands with `cargo --list`",
                );
                return Err(err.into());
            }
        }
        Err(FindError::UnknownSubcommand {
            valid_prefix,
            invalid,
        }) => {
            let valid_prefix = valid_prefix.join(" ");
            let err = anyhow::format_err!(
                "no such command: `cargo {valid_prefix} {invalid}` \n\n\
                 help: view available subcommands with `cargo {valid_prefix} --help`",
            );
            return Err(err.into());
        }
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

enum FindError<'a> {
    /// The primary command was not found.
    UnknownCommand(&'a str),
    /// A subcommand in the path was not found.
    UnknownSubcommand {
        valid_prefix: Vec<&'a str>,
        invalid: &'a str,
    },
}

/// Finds a auiltin command.
fn find_builtin_cmd<'a>(parts: &[&'a str]) -> Result<Vec<String>, FindError<'a>> {
    let Some((first, rest)) = parts.split_first() else {
        return Err(FindError::UnknownCommand(""));
    };

    let builtins = super::builtin();

    let Some(mut current) = builtins
        .iter()
        .find(|cmd| cmd.get_name() == *first || cmd.get_all_aliases().any(|a| a == *first))
    else {
        return Err(FindError::UnknownCommand(first));
    };

    let mut path = vec![current.get_name().to_string()];

    for (i, &part) in rest.iter().enumerate() {
        let next = current
            .get_subcommands()
            .find(|cmd| cmd.get_name() == part || cmd.get_all_aliases().any(|a| a == part));
        if let Some(next) = next {
            path.push(next.get_name().to_string());
            current = next;
        } else {
            let valid_prefix = [*first]
                .into_iter()
                .chain(rest[..i].iter().copied())
                .collect::<Vec<_>>();
            return Err(FindError::UnknownSubcommand {
                valid_prefix,
                invalid: part,
            });
        };
    }

    Ok(path)
}

fn find_builtin_cmd_dash_joined(s: &str) -> Option<String> {
    let builtins = super::builtin();

    for cmd in builtins.iter() {
        if let Some(result) = try_match_cmd(cmd, s) {
            return Some(result);
        }
    }
    None
}

/// Tries to match a single dash-joined argument against commands
fn try_match_cmd(cmd: &Command, arg: &str) -> Option<String> {
    let name = cmd.get_name();

    if arg == name || cmd.get_all_aliases().any(|alias| alias == arg) {
        return Some(name.to_string());
    }

    if let Some(rest) = arg.strip_prefix(name).and_then(|r| r.strip_prefix('-')) {
        for cmd in cmd.get_subcommands() {
            if let Some(sub_cmds) = try_match_cmd(cmd, rest) {
                return Some(format!("{name}-{sub_cmds}"));
            }
        }
    }

    for alias in cmd.get_all_aliases() {
        if let Some(rest) = arg.strip_prefix(alias).and_then(|r| r.strip_prefix('-')) {
            for cmd in cmd.get_subcommands() {
                if let Some(sub_cmds) = try_match_cmd(cmd, rest) {
                    return Some(format!("{name}-{sub_cmds}"));
                }
            }
        }
    }

    None
}
