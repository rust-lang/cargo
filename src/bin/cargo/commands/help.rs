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
        .arg(Arg::new("COMMAND").action(ArgAction::Set).add(
            clap_complete::ArgValueCandidates::new(|| {
                super::builtin()
                    .iter()
                    .map(|cmd| {
                        let name = cmd.get_name();
                        clap_complete::CompletionCandidate::new(name)
                            .help(cmd.get_about().cloned())
                            .hide(cmd.is_hide_set())
                    })
                    .collect()
            }),
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let subcommand = args.get_one::<String>("COMMAND");
    if let Some(subcommand) = subcommand {
        if !try_help(gctx, subcommand)? {
            match check_builtin(&subcommand) {
                Some(s) => {
                    crate::execute_internal_subcommand(
                        gctx,
                        &[OsStr::new(s), OsStr::new("--help")],
                    )?;
                }
                None => {
                    crate::execute_external_subcommand(
                        gctx,
                        subcommand,
                        &[OsStr::new(subcommand), OsStr::new("--help")],
                    )?;
                }
            }
        }
    } else {
        let mut cmd = crate::cli::cli(gctx);
        let _ = cmd.print_help();
    }
    Ok(())
}

fn try_help(gctx: &GlobalContext, subcommand: &str) -> CargoResult<bool> {
    let subcommand = match check_alias(gctx, subcommand) {
        // If this alias is more than a simple subcommand pass-through, show the alias.
        Some(argv) if argv.len() > 1 => {
            let alias = argv.join(" ");
            drop_println!(gctx, "`{}` is aliased to `{}`", subcommand, alias);
            return Ok(true);
        }
        // Otherwise, resolve the alias into its subcommand.
        Some(argv) => {
            // An alias with an empty argv can be created via `"empty-alias" = ""`.
            let first = argv.get(0).map(String::as_str).unwrap_or(subcommand);
            first.to_string()
        }
        None => subcommand.to_string(),
    };

    let subcommand = match check_builtin(&subcommand) {
        Some(s) => s,
        None => return Ok(false),
    };

    if resolve_executable(Path::new("man")).is_ok() {
        let man = match extract_man(subcommand, "1") {
            Some(man) => man,
            None => return Ok(false),
        };
        write_and_spawn(subcommand, &man, "man")?;
    } else {
        let txt = match extract_man(subcommand, "txt") {
            Some(txt) => txt,
            None => return Ok(false),
        };
        if resolve_executable(Path::new("less")).is_ok() {
            write_and_spawn(subcommand, &txt, "less")?;
        } else if resolve_executable(Path::new("more")).is_ok() {
            write_and_spawn(subcommand, &txt, "more")?;
        } else {
            drop(std::io::stdout().write_all(&txt));
        }
    }
    Ok(true)
}

/// Checks if the given subcommand is an alias.
///
/// Returns None if it is not an alias.
fn check_alias(gctx: &GlobalContext, subcommand: &str) -> Option<Vec<String>> {
    aliased_command(gctx, subcommand).ok().flatten()
}

/// Checks if the given subcommand is a built-in command (not via an alias).
///
/// Returns None if it is not a built-in command.
fn check_builtin(subcommand: &str) -> Option<&str> {
    super::builtin_exec(subcommand).map(|_| subcommand)
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
