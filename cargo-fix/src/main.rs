#[macro_use]
extern crate failure;
extern crate rustfix;
extern crate serde_json;
#[macro_use]
extern crate log;

use std::collections::{HashSet, HashMap};
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::process::{self, Command, ExitStatus};
use std::str;
use std::path::Path;

use rustfix::diagnostics::Diagnostic;
use failure::{Error, ResultExt};

mod lock;

fn main() {
    let result = if env::var("__CARGO_FIX_NOW_RUSTC").is_ok() {
        cargo_fix_rustc()
    } else {
        cargo_fix()
    };
    let err = match result {
        Ok(()) => return,
        Err(e) => e,
    };
    eprintln!("error: {}", err);
    for cause in err.causes().skip(1) {
        eprintln!("\tcaused by: {}", cause);
    }
    process::exit(102);
}

fn cargo_fix() -> Result<(), Error> {
    // Spin up our lock server which our subprocesses will use to synchronize
    // fixes.
    let _lockserver = lock::Server::new()?.start()?;

    let cargo = env::var_os("CARGO").unwrap_or("cargo".into());
    let mut cmd = Command::new(&cargo);
    // TODO: shouldn't hardcode `check` here, we want to allow things like
    // `cargo fix bench` or something like that
    //
    // TODO: somehow we need to force `check` to actually do something here, if
    // `cargo check` was previously run it won't actually do anything again.
    cmd.arg("check");
    cmd.args(env::args().skip(2)); // skip `cmd-fix fix`

    // Override the rustc compiler as ourselves. That way whenever rustc would
    // run we run instead and have an opportunity to inject fixes.
    let me = env::current_exe()
        .with_context(|_| "failed to learn about path to current exe")?;
    cmd.env("RUSTC", &me)
        .env("__CARGO_FIX_NOW_RUSTC", "1");
    if let Some(rustc) = env::var_os("RUSTC") {
        cmd.env("RUSTC_ORIGINAL", rustc);
    }

    // An now execute all of Cargo! This'll fix everything along the way.
    //
    // TODO: we probably want to do something fancy here like collect results
    // from the client processes and print out a summary of what happened.
    let status = cmd.status()
        .with_context(|e| {
            format!("failed to execute `{}`: {}", cargo.to_string_lossy(), e)
        })?;
    exit_with(status);
}

fn cargo_fix_rustc() -> Result<(), Error> {
    // Try to figure out what we're compiling by looking for a rust-like file
    // that exists.
    let filename = env::args()
        .skip(1)
        .filter(|s| s.ends_with(".rs"))
        .filter(|s| Path::new(s).exists())
        .next();

    let rustc = env::var_os("RUSTC_ORIGINAL").unwrap_or("rustc".into());

    // Our goal is to fix only the crates that the end user is interested in.
    // That's very likely to only mean the crates in the workspace the user is
    // working on, not random crates.io crates.
    //
    // To that end we only actually try to fix things if it looks like we're
    // compiling a Rust file and it *doesn't* have an absolute filename. That's
    // not the best heuristic but matches what Cargo does today at least.
    if let Some(path) = filename {
        if !Path::new(&path).is_absolute() {
            rustfix_crate(rustc.as_ref(), &path)?;
        }
    }

    // TODO: if we executed rustfix above and the previous rustc invocation was
    // successful and this `status()` is not, then we should revert all fixes
    // we applied, present a scary warning, and then move on.
    let mut cmd = Command::new(&rustc);
    cmd.args(env::args().skip(1));
    exit_with(cmd.status().with_context(|_| "failed to spawn rustc")?);

}

fn rustfix_crate(rustc: &Path, filename: &str) -> Result<(), Error> {
    // First up we want to make sure that each crate is only checked by one
    // process at a time. If two invocations concurrently check a crate then
    // it's likely to corrupt it.
    //
    // Currently we do this by assigning the name on our lock to the first
    // argument that looks like a Rust file.
    let _lock = lock::Client::lock(filename)?;

    let mut cmd = Command::new(&rustc);
    cmd.args(env::args().skip(1));
    cmd.arg("--error-format=json");
    let context = format!("failed to execute `{}`", rustc.to_string_lossy());
    let output = cmd.output().context(context.clone())?;

    // Sift through the output of the compiler to look for JSON messages
    // indicating fixes that we can apply. Note that we *do not* look at the
    // exit status here, that's intentional! We want to apply fixes even if
    // there are compiler errors.
    let stderr = str::from_utf8(&output.stderr)
        .map_err(|_| format_err!("failed to parse rustc stderr as utf-8"))?;
    let only = HashSet::new();
    let suggestions = stderr.lines()
        .filter(|x| !x.is_empty())

        // Parse each line of stderr ignoring errors as they may not all be json
        .filter_map(|line| serde_json::from_str::<Diagnostic>(line).ok())

        // From each diagnostic try to extract suggestions from rustc
        .filter_map(|diag| rustfix::collect_suggestions(&diag, &only));

    // Collect suggestions by file so we can apply them one at a time later.
    let mut file_map = HashMap::new();
    for suggestion in suggestions {
        // Make sure we've got a file associated with this suggestion and all
        // snippets point to the same location. Right now it's not clear what
        // we would do with multiple locations.
        let (file_name, range) = match suggestion.snippets.get(0) {
            Some(s) => (s.file_name.clone(), s.line_range),
            None => continue,
        };
        if !suggestion.snippets.iter().all(|s| {
            s.file_name == file_name && s.line_range == range
        }) {
            continue
        }

        file_map.entry(file_name)
            .or_insert_with(Vec::new)
            .push(suggestion);
    }

    for (file, suggestions) in file_map {
        // Attempt to read the source code for this file. If this fails then
        // that'd be pretty surprising, so log a message and otherwise keep
        // going.
        let mut code = String::new();
        if let Err(e) = File::open(&file).and_then(|mut f| f.read_to_string(&mut code)) {
            warn!("failed to read `{}`: {}", file, e);
            continue
        }
        let new_code = rustfix::apply_suggestions(&code, &suggestions);
        File::create(&file)
            .and_then(|mut f| f.write_all(new_code.as_bytes()))
            .with_context(|_| format!("failed to write file `{}`", file))?;
    }

    Ok(())
}

fn exit_with(status: ExitStatus) -> ! {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::*;
        if let Some(signal) = status.signal() {
            eprintln!("child failed with signal `{}`", signal);
            process::exit(2);
        }
    }
    process::exit(status.code().unwrap_or(3));
}
