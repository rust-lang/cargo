use std::env;
use std::io::Write;
use std::process::Command;

use clap::{App, AppSettings, Arg, SubCommand};
use failure::{Error, ResultExt};
use termcolor::{ColorSpec, StandardStream, WriteColor};

use super::exit_with;
use diagnostics::{self, log_for_human, output_stream, write_warning, Message};
use lock;
use vcs::VersionControl;

static PLEASE_REPORT_THIS_BUG: &str =
    "\
     This likely indicates a bug in either rustc or rustfix itself,\n\
     and we would appreciate a bug report! You're likely to see \n\
     a number of compiler warnings after this message which rustfix\n\
     attempted to fix but failed. If you could open an issue at\n\
     https://github.com/rust-lang-nursery/rustfix/issues\n\
     quoting the full output of this command we'd be very appreciative!\n\n\
     ";

pub fn run() -> Result<(), Error> {
    let matches = App::new("Cargo Fix")
        .bin_name("cargo")
        .subcommand(
            SubCommand::with_name("fix")
                .version(env!("CARGO_PKG_VERSION"))
                .author("The Rust Project Developers")
                .about("Automatically apply rustc's suggestions about fixing code")
                .setting(AppSettings::TrailingVarArg)
                .arg(Arg::with_name("args").multiple(true))
                .arg(
                    Arg::with_name("broken-code")
                        .long("broken-code")
                        .help("Fix code even if it already has compiler errors"),
                )
                .arg(
                    Arg::with_name("edition")
                        .long("prepare-for")
                        .help("Fix warnings in preparation of an edition upgrade")
                        .takes_value(true)
                        .possible_values(&["2018"]),
                )
                .arg(
                    Arg::with_name("allow-no-vcs")
                        .long("allow-no-vcs")
                        .help("Fix code even if a VCS was not detected"),
                )
                .arg(
                    Arg::with_name("allow-dirty")
                        .long("allow-dirty")
                        .help("Fix code even if the working directory is dirty"),
                ),
        )
        .get_matches();

    let matches = match matches.subcommand() {
        ("fix", Some(matches)) => matches,
        _ => bail!("unknown CLI arguments passed"),
    };

    if matches.is_present("broken-code") {
        env::set_var("__CARGO_FIX_BROKEN_CODE", "1");
    }

    check_version_control(matches)?;

    // Spin up our lock server which our subprocesses will use to synchronize
    // fixes.
    let _lock_server = lock::Server::new()?.start()?;

    // Spin up our diagnostics server which our subprocesses will use to send
    // use their dignostics messages in an ordered way.
    let _diagnostics_server = diagnostics::Server::new()?.start(|m, stream| {
        if let Err(e) = log_message(&m, stream) {
            warn!("failed to log message: {}", e);
        }
    })?;

    let cargo = env::var_os("CARGO").unwrap_or("cargo".into());
    let mut cmd = Command::new(&cargo);
    // TODO: shouldn't hardcode `check` here, we want to allow things like
    // `cargo fix bench` or something like that
    //
    // TODO: somehow we need to force `check` to actually do something here, if
    // `cargo check` was previously run it won't actually do anything again.
    cmd.arg("check");
    if let Some(args) = matches.values_of("args") {
        cmd.args(args);
    }

    // Override the rustc compiler as ourselves. That way whenever rustc would
    // run we run instead and have an opportunity to inject fixes.
    let me = env::current_exe().context("failed to learn about path to current exe")?;
    cmd.env("RUSTC", &me).env("__CARGO_FIX_NOW_RUSTC", "1");
    if let Some(rustc) = env::var_os("RUSTC") {
        cmd.env("RUSTC_ORIGINAL", rustc);
    }

    // Trigger edition-upgrade mode. Currently only supports the 2018 edition.
    info!("edition upgrade? {:?}", matches.value_of("edition"));
    if let Some("2018") = matches.value_of("edition") {
        info!("edition upgrade!");
        let mut rustc_flags = env::var_os("RUSTFLAGS").unwrap_or_else(|| "".into());
        rustc_flags.push("-W rust-2018-compatibility");
        cmd.env("RUSTFLAGS", &rustc_flags);
    }

    // An now execute all of Cargo! This'll fix everything along the way.
    //
    // TODO: we probably want to do something fancy here like collect results
    // from the client processes and print out a summary of what happened.
    let status = cmd.status()
        .with_context(|e| format!("failed to execute `{}`: {}", cargo.to_string_lossy(), e))?;
    exit_with(status);
}

fn check_version_control(matches: &::clap::ArgMatches) -> Result<(), Error> {
    // Useful for tests
    if env::var("__CARGO_FIX_IGNORE_VCS").is_ok() {
        return Ok(());
    }

    let version_control = VersionControl::new();
    match (version_control.is_present(), version_control.is_dirty()?) {
        (true, None) => {} // clean and versioned slate
        (false, _) => {
            let stream = &mut output_stream();

            write_warning(stream)?;
            stream.set_color(ColorSpec::new().set_bold(true))?;
            writeln!(stream, "Could not detect a version control system")?;
            stream.reset()?;
            writeln!(stream, "You should consider using a VCS so you can easily see and revert rustfix's changes.")?;

            if !matches.is_present("allow-no-vcs") {
                bail!("No VCS found, aborting. Overwrite this behavior with `--allow-no-vcs`.");
            }
        }
        (true, Some(output)) => {
            let stream = &mut output_stream();

            write_warning(stream)?;
            stream.set_color(ColorSpec::new().set_bold(true))?;
            writeln!(stream, "Working directory dirty")?;
            stream.reset()?;
            writeln!(stream, "Make sure your working directory is clean so you can easily revert rustfix's changes.")?;

            stream.write_all(&output)?;

            if !matches.is_present("allow-dirty") {
                bail!("Aborting because of dirty working directory. Overwrite this behavior with `--allow-dirty`.");
            }
        }
    }

    Ok(())
}

fn log_message(msg: &Message, stream: &mut StandardStream) -> Result<(), Error> {
    use diagnostics::Message::*;

    match *msg {
        Fixing {
            ref file,
            ref fixes,
        } => {
            log_for_human(
                "Fixing",
                &format!(
                    "{name} ({n} {fixes})",
                    name = file,
                    n = fixes,
                    fixes = if *fixes > 1 { "fixes" } else { "fix" },
                ),
                stream,
            )?;
        }
        ReplaceFailed {
            ref file,
            ref message,
        } => {
            write_warning(stream)?;
            stream.set_color(ColorSpec::new().set_bold(true))?;
            write!(stream, "error applying suggestions to `{}`\n", file)?;
            stream.reset()?;
            write!(stream, "The full error message was:\n\n> {}\n\n", message)?;
            stream.write(PLEASE_REPORT_THIS_BUG.as_bytes())?;
        }
        FixFailed {
            ref files,
            ref krate,
        } => {
            write_warning(stream)?;
            stream.set_color(ColorSpec::new().set_bold(true))?;
            if let Some(ref krate) = *krate {
                write!(
                    stream,
                    "failed to automatically apply fixes suggested by rustc \
                     to crate `{}`\n",
                    krate,
                )?;
            } else {
                write!(
                    stream,
                    "failed to automatically apply fixes suggested by rustc\n"
                )?;
            }
            if files.len() > 0 {
                write!(
                    stream,
                    "\nafter fixes were automatically applied the compiler \
                     reported errors within these files:\n\n"
                )?;
                for file in files {
                    write!(stream, "  * {}\n", file)?;
                }
                write!(stream, "\n")?;
            }
            stream.write(PLEASE_REPORT_THIS_BUG.as_bytes())?;
        }
    }

    stream.reset()?;
    stream.flush()?;
    Ok(())
}
