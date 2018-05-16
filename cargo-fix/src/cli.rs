use std::env;
use std::io::Write;
use std::process::Command;

use clap::{App, AppSettings, Arg, SubCommand};
use failure::{Error, ResultExt};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

use super::exit_with;
use diagnostics::{Message, Server};
use lock;

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
                ),
        )
        .get_matches();
    let matches = match matches.subcommand() {
        ("fix", Some(matches)) => matches,
        _ => bail!("unknown cli arguments passed"),
    };

    if matches.is_present("broken-code") {
        env::set_var("__CARGO_FIX_BROKEN_CODE", "1");
    }

    // Spin up our lock server which our subprocesses will use to synchronize
    // fixes.
    let _lockserver = lock::Server::new()?.start()?;

    // Spin up our diagnostics server which our subprocesses will use to send
    // use their dignostics messages in an ordered way.
    let _lockserver = Server::new()?.start(|m, stream| {
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
            stream.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Yellow)))?;
            write!(stream, "warning")?;
            stream.reset()?;
            stream.set_color(ColorSpec::new().set_bold(true))?;
            write!(stream, ": error applying suggestions to `{}`\n", file)?;
            stream.reset()?;
            write!(stream, "The full error message was:\n\n> {}\n\n", message)?;
            stream.write(PLEASE_REPORT_THIS_BUG.as_bytes())?;
        }
        FixFailed {
            ref files,
            ref krate,
        } => {
            stream.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Yellow)))?;
            write!(stream, "warning")?;
            stream.reset()?;
            stream.set_color(ColorSpec::new().set_bold(true))?;
            write!(stream, ": ")?;
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

fn log_for_human(kind: &str, msg: &str, stream: &mut StandardStream) -> Result<(), Error> {
    stream.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Cyan)))?;
    // Justify to 12 chars just like cargo
    write!(stream, "{:>12}", kind)?;
    stream.reset()?;
    write!(stream, " {}\n", msg)?;
    Ok(())
}
