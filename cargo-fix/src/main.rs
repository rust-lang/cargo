extern crate clap;
#[macro_use]
extern crate failure;
extern crate rustfix;
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate env_logger;

use std::collections::{HashSet, HashMap};
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::process::{self, Command, ExitStatus};
use std::str;
use std::path::Path;

use rustfix::diagnostics::Diagnostic;
use failure::{Error, ResultExt};

mod cli;
mod lock;

fn main() {
    env_logger::init();
    let result = if env::var("__CARGO_FIX_NOW_RUSTC").is_ok() {
        debug!("invoking cargo-fix as rustc wrapper");
        cargo_fix_rustc()
    } else {
        debug!("invoking cargo-fix as cargo subcommand");
        cli::run()
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

fn cargo_fix_rustc() -> Result<(), Error> {
    // Try to figure out what we're compiling by looking for a rust-like file
    // that exists.
    let filename = env::args()
        .skip(1)
        .filter(|s| s.ends_with(".rs"))
        .filter(|s| Path::new(s).exists())
        .next();

    trace!("cargo-fix as rustc got file {:?}", filename);
    let rustc = env::var_os("RUSTC_ORIGINAL").unwrap_or("rustc".into());

    // Our goal is to fix only the crates that the end user is interested in.
    // That's very likely to only mean the crates in the workspace the user is
    // working on, not random crates.io crates.
    //
    // To that end we only actually try to fix things if it looks like we're
    // compiling a Rust file and it *doesn't* have an absolute filename. That's
    // not the best heuristic but matches what Cargo does today at least.
    let mut files_to_restore = HashMap::new();
    if let Some(path) = filename {
        if !Path::new(&path).is_absolute() {
            trace!("start rustfixing {:?}", path);
            files_to_restore = rustfix_crate(rustc.as_ref(), &path)?;
        }
    }

    // Ok now we have our final goal of testing out the changes that we applied.
    // If these changes went awry and actually started to cause the crate to
    // *stop* compiling then we want to back them out and continue to print
    // warnings to the user.
    //
    // If we didn't actually make any changes then we can immediately exec the
    // new rustc, and otherwise we capture the output to hide it in the scenario
    // that we have to back it all out.
    let mut cmd = Command::new(&rustc);
    cmd.args(env::args().skip(1));
    cmd.arg("--cap-lints=warn");
    if files_to_restore.len() > 0 {
        let output = cmd.output().context("failed to spawn rustc")?;

        // If we succeeded then we'll want to commit to the changes we made, if
        // any. If stderr is empty then there's no need for the final exec at
        // the end, we just bail out here.
        if output.status.success() && output.stderr.len() == 0 {
            return Ok(())
        }

        // Otherwise if our rustc just failed then that means that we broke the
        // user's code with our changes. Back out everything and fall through
        // below to recompile again.
        if !output.status.success() {
            for (k, v) in files_to_restore {
                File::create(&k)
                    .and_then(|mut f| f.write_all(v.as_bytes()))
                    .with_context(|_| format!("failed to write file `{}`", k))?;
            }
        }
    }

    exit_with(cmd.status().context("failed to spawn rustc")?);
}

fn rustfix_crate(rustc: &Path, filename: &str) -> Result<HashMap<String, String>, Error> {
    // If not empty, filter by these lints
    //
    // TODO: Implement a way to specify this
    let only = HashSet::new();

    // First up we want to make sure that each crate is only checked by one
    // process at a time. If two invocations concurrently check a crate then
    // it's likely to corrupt it.
    //
    // Currently we do this by assigning the name on our lock to the first
    // argument that looks like a Rust file.
    let _lock = lock::Client::lock(filename)?;

    let mut cmd = Command::new(&rustc);
    cmd.args(env::args().skip(1));
    cmd.arg("--error-format=json")
        .arg("--cap-lints=warn");
    let output = cmd.output()
        .with_context(|_| format!("failed to execute `{}`", rustc.display()))?;

    // If rustc didn't succeed for whatever reasons then we're very likely to be
    // looking at otherwise broken code. Let's not make things accidentally
    // worse by applying fixes where a bug could cause *more* broken code.
    // Instead, punt upwards which will reexec rustc over the original code,
    // displaying pretty versions of the diagnostics we just read out.
    //
    // TODO: this should be configurable by the CLI to sometimes proceed to
    // attempt to fix broken code.
    if !output.status.success() && env::var_os("__CARGO_FIX_BROKEN_CODE").is_none() {
        debug!("rustfixing `{:?}` failed, rustc exited with {:?}", filename, output.status.code());
        return Ok(HashMap::new())
    }

    // Sift through the output of the compiler to look for JSON messages
    // indicating fixes that we can apply.
    let stderr = str::from_utf8(&output.stderr)
        .context("failed to parse rustc stderr as utf-8")?;

    let suggestions = stderr.lines()
        .filter(|x| !x.is_empty())

        // Parse each line of stderr ignoring errors as they may not all be json
        .filter_map(|line| serde_json::from_str::<Diagnostic>(line).ok())

        // From each diagnostic try to extract suggestions from rustc
        .filter_map(|diag| rustfix::collect_suggestions(&diag, &only));

    // Collect suggestions by file so we can apply them one at a time later.
    let mut file_map = HashMap::new();
    let mut num_suggestion = 0;
    for suggestion in suggestions {
        // Make sure we've got a file associated with this suggestion and all
        // snippets point to the same location. Right now it's not clear what
        // we would do with multiple locations.
        let (file_name, range) = match suggestion.snippets.get(0) {
            Some(s) => (s.file_name.clone(), s.line_range),
            None => {
                trace!("rejecting as it has no snippets {:?}", suggestion);
                continue
            }
        };
        if !suggestion.snippets.iter().all(|s| {
            s.file_name == file_name && s.line_range == range
        }) {
            trace!("rejecting as it spans mutliple files {:?}", suggestion);
            continue
        }

        file_map.entry(file_name)
            .or_insert_with(Vec::new)
            .push(suggestion);
        num_suggestion += 1;
    }

    debug!("collected {} suggestions for `{}`", num_suggestion, filename);

    let mut old_files = HashMap::with_capacity(file_map.len());
    for (file, suggestions) in file_map {
        // Attempt to read the source code for this file. If this fails then
        // that'd be pretty surprising, so log a message and otherwise keep
        // going.
        let mut code = String::new();
        if let Err(e) = File::open(&file).and_then(|mut f| f.read_to_string(&mut code)) {
            warn!("failed to read `{}`: {}", file, e);
            continue
        }
        debug!("applying {} fixes to {}", suggestions.len(), file);

        let new_code = rustfix::apply_suggestions(&code, &suggestions)?;
        File::create(&file)
            .and_then(|mut f| f.write_all(new_code.as_bytes()))
            .with_context(|_| format!("failed to write file `{}`", file))?;
        old_files.insert(file, code);
    }

    Ok(old_files)
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
