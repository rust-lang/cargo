use std::collections::{HashMap, HashSet, BTreeSet};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{self, Command, ExitStatus};
use std::str;

use failure::{Error, ResultExt};
use git2;
use rustfix::diagnostics::Diagnostic;
use rustfix;
use serde_json;

use core::Workspace;
use ops::{self, CompileOptions};
use ops::cargo_new::existing_vcs_repo;
use util::errors::CargoResult;
use util::{LockServer, LockServerClient};
use util::diagnostic_server::{Message, RustfixDiagnosticServer};
use util::paths;

const FIX_ENV: &str = "__CARGO_FIX_PLZ";
const BROKEN_CODE_ENV: &str = "__CARGO_FIX_BROKEN_CODE";

pub struct FixOptions<'a> {
    pub edition: Option<&'a str>,
    pub compile_opts: CompileOptions<'a>,
    pub allow_dirty: bool,
    pub allow_no_vcs: bool,
    pub broken_code: bool,
}

pub fn fix(ws: &Workspace, opts: &mut FixOptions) -> CargoResult<()> {
    check_version_control(opts)?;

    // Spin up our lock server which our subprocesses will use to synchronize
    // fixes.
    let lock_server = LockServer::new()?;
    opts.compile_opts.build_config.extra_rustc_env.push((
        FIX_ENV.to_string(),
        lock_server.addr().to_string(),
    ));
    let _started = lock_server.start()?;

    if opts.broken_code {
        let key = BROKEN_CODE_ENV.to_string();
        opts.compile_opts.build_config.extra_rustc_env.push((key, "1".to_string()));
    }

    if let Some(edition) = opts.edition {
        opts.compile_opts.build_config.extra_rustc_args.push("-W".to_string());
        let lint_name = format!("rust-{}-compatibility", edition);
        opts.compile_opts.build_config.extra_rustc_args.push(lint_name);
    }
    opts.compile_opts.build_config.cargo_as_rustc_wrapper = true;
    *opts.compile_opts.build_config.rustfix_diagnostic_server.borrow_mut() =
        Some(RustfixDiagnosticServer::new()?);

    ops::compile(ws, &opts.compile_opts)?;
    Ok(())
}

fn check_version_control(opts: &FixOptions) -> CargoResult<()> {
    if opts.allow_no_vcs {
        return Ok(())
    }
    let config = opts.compile_opts.config;
    if !existing_vcs_repo(config.cwd(), config.cwd()) {
        bail!("no VCS found for this project and `cargo fix` can potentially \
               perform destructive changes; if you'd like to suppress this \
               error pass `--allow-no-vcs`")
    }

    if opts.allow_dirty {
        return Ok(())
    }

    let mut dirty_files = Vec::new();
    if let Ok(repo) = git2::Repository::discover(config.cwd()) {
        for status in repo.statuses(None)?.iter() {
            if status.status() != git2::Status::CURRENT {
                if let Some(path) = status.path() {
                    dirty_files.push(path.to_string());
                }
            }

        }
    }

    if dirty_files.len() == 0 {
        return Ok(())
    }

    let mut files_list = String::new();
    for file in dirty_files {
        files_list.push_str("  * ");
        files_list.push_str(&file);
        files_list.push_str("\n");
    }

    bail!("the working directory of this project is detected as dirty, and \
           `cargo fix` can potentially perform destructive changes; if you'd \
           like to suppress this error pass `--allow-dirty`, or commit the \
           changes to these files:\n\
           \n\
           {}\n\
          ", files_list);
}

pub fn fix_maybe_exec_rustc() -> CargoResult<bool> {
    let lock_addr = match env::var(FIX_ENV) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    // Try to figure out what we're compiling by looking for a rust-like file
    // that exists.
    let filename = env::args()
        .skip(1)
        .filter(|s| s.ends_with(".rs"))
        .filter(|s| Path::new(s).exists())
        .next();

    trace!("cargo-fix as rustc got file {:?}", filename);
    let rustc = env::var_os("RUSTC").expect("failed to find RUSTC env var");

    // Our goal is to fix only the crates that the end user is interested in.
    // That's very likely to only mean the crates in the workspace the user is
    // working on, not random crates.io crates.
    //
    // To that end we only actually try to fix things if it looks like we're
    // compiling a Rust file and it *doesn't* have an absolute filename. That's
    // not the best heuristic but matches what Cargo does today at least.
    let mut fixes = FixedCrate::default();
    if let Some(path) = filename {
        if !Path::new(&path).is_absolute() {
            trace!("start rustfixing {:?}", path);
            fixes = rustfix_crate(&lock_addr, rustc.as_ref(), &path)?;
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
    cmd.arg("--error-format=json");
    if fixes.original_files.len() > 0 {
        let output = cmd.output().context("failed to spawn rustc")?;

        if output.status.success() {
            for message in fixes.messages.drain(..) {
                message.post()?;
            }
        }

        // If we succeeded then we'll want to commit to the changes we made, if
        // any. If stderr is empty then there's no need for the final exec at
        // the end, we just bail out here.
        if output.status.success() && output.stderr.len() == 0 {
            return Ok(true);
        }

        // Otherwise if our rustc just failed then that means that we broke the
        // user's code with our changes. Back out everything and fall through
        // below to recompile again.
        if !output.status.success() {
            for (k, v) in fixes.original_files {
                File::create(&k)
                    .and_then(|mut f| f.write_all(v.as_bytes()))
                    .with_context(|_| format!("failed to write file `{}`", k))?;
            }
            log_failed_fix(&output.stderr)?;
        }
    }

    let mut cmd = Command::new(&rustc);
    cmd.args(env::args().skip(1));
    cmd.arg("--cap-lints=warn");
    exit_with(cmd.status().context("failed to spawn rustc")?);
}

#[derive(Default)]
struct FixedCrate {
    messages: Vec<Message>,
    original_files: HashMap<String, String>,
}

fn rustfix_crate(lock_addr: &str, rustc: &Path, filename: &str)
    -> Result<FixedCrate, Error>
{
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
    let _lock = LockServerClient::lock(&lock_addr.parse()?, filename)?;

    let mut cmd = Command::new(&rustc);
    cmd.args(env::args().skip(1));
    cmd.arg("--error-format=json").arg("--cap-lints=warn");
    let output = cmd.output()
        .with_context(|_| format!("failed to execute `{}`", rustc.display()))?;

    // If rustc didn't succeed for whatever reasons then we're very likely to be
    // looking at otherwise broken code. Let's not make things accidentally
    // worse by applying fixes where a bug could cause *more* broken code.
    // Instead, punt upwards which will reexec rustc over the original code,
    // displaying pretty versions of the diagnostics we just read out.
    if !output.status.success() && env::var_os(BROKEN_CODE_ENV).is_none() {
        debug!(
            "rustfixing `{:?}` failed, rustc exited with {:?}",
            filename,
            output.status.code()
        );
        return Ok(Default::default());
    }

    let fix_mode = env::var_os("__CARGO_FIX_YOLO")
        .map(|_| rustfix::Filter::Everything)
        .unwrap_or(rustfix::Filter::MachineApplicableOnly);

    // Sift through the output of the compiler to look for JSON messages
    // indicating fixes that we can apply.
    let stderr = str::from_utf8(&output.stderr).context("failed to parse rustc stderr as utf-8")?;

    let suggestions = stderr.lines()
        .filter(|x| !x.is_empty())
        .inspect(|y| trace!("line: {}", y))

        // Parse each line of stderr ignoring errors as they may not all be json
        .filter_map(|line| serde_json::from_str::<Diagnostic>(line).ok())

        // From each diagnostic try to extract suggestions from rustc
        .filter_map(|diag| rustfix::collect_suggestions(&diag, &only, fix_mode));

    // Collect suggestions by file so we can apply them one at a time later.
    let mut file_map = HashMap::new();
    let mut num_suggestion = 0;
    for suggestion in suggestions {
        trace!("suggestion");
        // Make sure we've got a file associated with this suggestion and all
        // snippets point to the same location. Right now it's not clear what
        // we would do with multiple locations.
        let (file_name, range) = match suggestion.snippets.get(0) {
            Some(s) => (s.file_name.clone(), s.line_range),
            None => {
                trace!("rejecting as it has no snippets {:?}", suggestion);
                continue;
            }
        };
        if !suggestion
            .snippets
            .iter()
            .all(|s| s.file_name == file_name && s.line_range == range)
        {
            trace!("rejecting as it spans multiple files {:?}", suggestion);
            continue;
        }

        file_map
            .entry(file_name)
            .or_insert_with(Vec::new)
            .push(suggestion);
        num_suggestion += 1;
    }

    debug!(
        "collected {} suggestions for `{}`",
        num_suggestion, filename
    );

    let mut original_files = HashMap::with_capacity(file_map.len());
    let mut messages = Vec::new();
    for (file, suggestions) in file_map {
        // Attempt to read the source code for this file. If this fails then
        // that'd be pretty surprising, so log a message and otherwise keep
        // going.
        let code = match paths::read(file.as_ref()) {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to read `{}`: {}", file, e);
                continue;
            }
        };
        let num_suggestions = suggestions.len();
        debug!("applying {} fixes to {}", num_suggestions, file);

        messages.push(Message::fixing(&file, num_suggestions));

        match rustfix::apply_suggestions(&code, &suggestions) {
            Err(e) => {
                Message::ReplaceFailed {
                    file: file,
                    message: e.to_string(),
                }.post()?;
                // TODO: Add flag to decide if we want to continue or bail out
                continue;
            }
            Ok(new_code) => {
                File::create(&file)
                    .and_then(|mut f| f.write_all(new_code.as_bytes()))
                    .with_context(|_| format!("failed to write file `{}`", file))?;
                original_files.insert(file, code);
            }
        }
    }

    Ok(FixedCrate {
        messages,
        original_files,
    })
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

fn log_failed_fix(stderr: &[u8]) -> Result<(), Error> {
    let stderr = str::from_utf8(stderr).context("failed to parse rustc stderr as utf-8")?;

    let diagnostics = stderr
        .lines()
        .filter(|x| !x.is_empty())
        .filter_map(|line| serde_json::from_str::<Diagnostic>(line).ok());
    let mut files = BTreeSet::new();
    for diagnostic in diagnostics {
        for span in diagnostic.spans.into_iter() {
            files.insert(span.file_name);
        }
    }
    let mut krate = None;
    let mut prev_dash_dash_krate_name = false;
    for arg in env::args() {
        if prev_dash_dash_krate_name {
            krate = Some(arg.clone());
        }

        if arg == "--crate-name" {
            prev_dash_dash_krate_name = true;
        } else {
            prev_dash_dash_krate_name = false;
        }
    }

    let files = files.into_iter().collect();
    Message::FixFailed { files, krate }.post()?;

    Ok(())
}
