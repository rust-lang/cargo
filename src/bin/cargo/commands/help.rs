use crate::aliased_command;
use cargo::util::errors::CargoResult;
use cargo::util::paths::resolve_executable;
use cargo::Config;
use flate2::read::GzDecoder;
use std::ffi::OsString;
use std::io::Read;
use std::io::Write;
use std::path::Path;

const COMPRESSED_MAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/man.tgz"));

/// Checks if the `help` command is being issued.
///
/// This runs before clap processing, because it needs to intercept the `help`
/// command if a man page is available.
///
/// Returns `true` if a man page was displayed. In this case, Cargo should
/// exit.
pub fn handle_embedded_help(config: &Config) -> bool {
    match try_help(config) {
        Ok(true) => true,
        Ok(false) => false,
        Err(e) => {
            log::warn!("man failed: {:?}", e);
            false
        }
    }
}

fn try_help(config: &Config) -> CargoResult<bool> {
    let mut args = std::env::args_os()
        .skip(1)
        .skip_while(|arg| arg.to_str().map_or(false, |s| s.starts_with('-')));
    if !args
        .next()
        .map_or(false, |arg| arg.to_str() == Some("help"))
    {
        return Ok(false);
    }
    let subcommand = match args.next() {
        Some(arg) => arg,
        None => return Ok(false),
    };
    let subcommand = match subcommand.to_str() {
        Some(s) => s,
        None => return Ok(false),
    };
    // Check if this is a built-in command (or alias);
    let subcommand = match check_alias(config, subcommand) {
        Some(s) => s,
        None => return Ok(false),
    };
    if resolve_executable(Path::new("man")).is_ok() {
        let man = match extract_man(&subcommand, "1") {
            Some(man) => man,
            None => return Ok(false),
        };
        write_and_spawn(&man, "man")?;
    } else {
        let txt = match extract_man(&subcommand, "txt") {
            Some(txt) => txt,
            None => return Ok(false),
        };
        if resolve_executable(Path::new("less")).is_ok() {
            write_and_spawn(&txt, "less")?;
        } else if resolve_executable(Path::new("more")).is_ok() {
            write_and_spawn(&txt, "more")?;
        } else {
            drop(std::io::stdout().write_all(&txt));
        }
    }
    Ok(true)
}

/// Checks if the given subcommand is a built-in command (possibly via an alias).
///
/// Returns None if it is not a built-in command.
fn check_alias(config: &Config, subcommand: &str) -> Option<String> {
    if super::builtin_exec(subcommand).is_some() {
        return Some(subcommand.to_string());
    }
    match aliased_command(config, subcommand) {
        Ok(Some(alias)) => {
            let alias = alias.into_iter().next()?;
            if super::builtin_exec(&alias).is_some() {
                Some(alias)
            } else {
                None
            }
        }
        _ => None,
    }
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
fn write_and_spawn(contents: &[u8], command: &str) -> CargoResult<()> {
    let mut tmp = tempfile::Builder::new().prefix("cargo-man").tempfile()?;
    let f = tmp.as_file_mut();
    f.write_all(&contents)?;
    f.flush()?;
    let mut cmd = std::process::Command::new(command)
        .arg(tmp.path())
        .spawn()?;
    drop(cmd.wait());
    Ok(())
}
