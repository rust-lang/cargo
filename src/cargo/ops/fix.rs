//! High-level overview of how `fix` works:
//!
//! The main goal is to run `cargo check` to get rustc to emit JSON
//! diagnostics with suggested fixes that can be applied to the files on the
//! filesystem, and validate that those changes didn't break anything.
//!
//! Cargo begins by launching a `LockServer` thread in the background to
//! listen for network connections to coordinate locking when multiple targets
//! are built simultaneously. It ensures each package has only one fix running
//! at once.
//!
//! The `RustfixDiagnosticServer` is launched in a background thread (in
//! `JobQueue`) to listen for network connections to coordinate displaying
//! messages to the user on the console (so that multiple processes don't try
//! to print at the same time).
//!
//! Cargo begins a normal `cargo check` operation with itself set as a proxy
//! for rustc by setting `cargo_as_rustc_wrapper` in the build config. When
//! cargo launches rustc to check a crate, it is actually launching itself.
//! The `FIX_ENV` environment variable is set so that cargo knows it is in
//! fix-proxy-mode. It also sets the `RUSTC` environment variable to the
//! actual rustc so Cargo knows what to execute.
//!
//! Each proxied cargo-as-rustc detects it is in fix-proxy-mode (via `FIX_ENV`
//! environment variable in `main`) and does the following:
//!
//! - Acquire a lock from the `LockServer` from the master cargo process.
//! - Launches the real rustc (`rustfix_and_fix`), looking at the JSON output
//!   for suggested fixes.
//! - Uses the `rustfix` crate to apply the suggestions to the files on the
//!   file system.
//! - If rustfix fails to apply any suggestions (for example, they are
//!   overlapping), but at least some suggestions succeeded, it will try the
//!   previous two steps up to 4 times as long as some suggestions succeed.
//! - Assuming there's at least one suggestion applied, and the suggestions
//!   applied cleanly, rustc is run again to verify the suggestions didn't
//!   break anything. The change will be backed out if it fails (unless
//!   `--broken-code` is used).
//! - If there are any warnings or errors, rustc will be run one last time to
//!   show them to the user.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitStatus};
use std::str;

use failure::{Error, ResultExt};
use log::{debug, trace, warn};
use rustfix::diagnostics::Diagnostic;
use rustfix::{self, CodeFix};

use crate::core::Workspace;
use crate::ops::{self, CompileOptions};
use crate::util::diagnostic_server::{Message, RustfixDiagnosticServer};
use crate::util::errors::CargoResult;
use crate::util::paths;
use crate::util::{existing_vcs_repo, LockServer, LockServerClient};

const FIX_ENV: &str = "__CARGO_FIX_PLZ";
const BROKEN_CODE_ENV: &str = "__CARGO_FIX_BROKEN_CODE";
const PREPARE_FOR_ENV: &str = "__CARGO_FIX_PREPARE_FOR";
const EDITION_ENV: &str = "__CARGO_FIX_EDITION";
const IDIOMS_ENV: &str = "__CARGO_FIX_IDIOMS";

pub struct FixOptions<'a> {
    pub edition: bool,
    pub prepare_for: Option<&'a str>,
    pub idioms: bool,
    pub compile_opts: CompileOptions<'a>,
    pub allow_dirty: bool,
    pub allow_no_vcs: bool,
    pub allow_staged: bool,
    pub broken_code: bool,
}

pub fn fix(ws: &Workspace<'_>, opts: &mut FixOptions<'_>) -> CargoResult<()> {
    check_version_control(opts)?;

    // Spin up our lock server, which our subprocesses will use to synchronize fixes.
    let lock_server = LockServer::new()?;
    opts.compile_opts
        .build_config
        .extra_rustc_env
        .push((FIX_ENV.to_string(), lock_server.addr().to_string()));
    let _started = lock_server.start()?;

    opts.compile_opts.build_config.force_rebuild = true;

    if opts.broken_code {
        let key = BROKEN_CODE_ENV.to_string();
        opts.compile_opts
            .build_config
            .extra_rustc_env
            .push((key, "1".to_string()));
    }

    if opts.edition {
        let key = EDITION_ENV.to_string();
        opts.compile_opts
            .build_config
            .extra_rustc_env
            .push((key, "1".to_string()));
    } else if let Some(edition) = opts.prepare_for {
        opts.compile_opts
            .build_config
            .extra_rustc_env
            .push((PREPARE_FOR_ENV.to_string(), edition.to_string()));
    }
    if opts.idioms {
        opts.compile_opts
            .build_config
            .extra_rustc_env
            .push((IDIOMS_ENV.to_string(), "1".to_string()));
    }
    opts.compile_opts.build_config.cargo_as_rustc_wrapper = true;
    *opts
        .compile_opts
        .build_config
        .rustfix_diagnostic_server
        .borrow_mut() = Some(RustfixDiagnosticServer::new()?);

    ops::compile(ws, &opts.compile_opts)?;
    Ok(())
}

fn check_version_control(opts: &FixOptions<'_>) -> CargoResult<()> {
    if opts.allow_no_vcs {
        return Ok(());
    }
    let config = opts.compile_opts.config;
    if !existing_vcs_repo(config.cwd(), config.cwd()) {
        failure::bail!(
            "no VCS found for this package and `cargo fix` can potentially \
             perform destructive changes; if you'd like to suppress this \
             error pass `--allow-no-vcs`"
        )
    }

    if opts.allow_dirty && opts.allow_staged {
        return Ok(());
    }

    let mut dirty_files = Vec::new();
    let mut staged_files = Vec::new();
    if let Ok(repo) = git2::Repository::discover(config.cwd()) {
        let mut repo_opts = git2::StatusOptions::new();
        repo_opts.include_ignored(false);
        for status in repo.statuses(Some(&mut repo_opts))?.iter() {
            if let Some(path) = status.path() {
                match status.status() {
                    git2::Status::CURRENT => (),
                    git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE => {
                        if !opts.allow_staged {
                            staged_files.push(path.to_string())
                        }
                    }
                    _ => {
                        if !opts.allow_dirty {
                            dirty_files.push(path.to_string())
                        }
                    }
                };
            }
        }
    }

    if dirty_files.is_empty() && staged_files.is_empty() {
        return Ok(());
    }

    let mut files_list = String::new();
    for file in dirty_files {
        files_list.push_str("  * ");
        files_list.push_str(&file);
        files_list.push_str(" (dirty)\n");
    }
    for file in staged_files {
        files_list.push_str("  * ");
        files_list.push_str(&file);
        files_list.push_str(" (staged)\n");
    }

    failure::bail!(
        "the working directory of this package has uncommitted changes, and \
         `cargo fix` can potentially perform destructive changes; if you'd \
         like to suppress this error pass `--allow-dirty`, `--allow-staged`, \
         or commit the changes to these files:\n\
         \n\
         {}\n\
         ",
        files_list
    );
}

pub fn fix_maybe_exec_rustc() -> CargoResult<bool> {
    let lock_addr = match env::var(FIX_ENV) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    let args = FixArgs::get();
    trace!("cargo-fix as rustc got file {:?}", args.file);
    let rustc = env::var_os("RUSTC").expect("failed to find RUSTC env var");

    // Our goal is to fix only the crates that the end user is interested in.
    // That's very likely to only mean the crates in the workspace the user is
    // working on, not random crates.io crates.
    //
    // To that end we only actually try to fix things if it looks like we're
    // compiling a Rust file and it *doesn't* have an absolute filename. That's
    // not the best heuristic but matches what Cargo does today at least.
    let mut fixes = FixedCrate::default();
    if let Some(path) = &args.file {
        if args.primary_package {
            trace!("start rustfixing {:?}", path);
            fixes = rustfix_crate(&lock_addr, rustc.as_ref(), path, &args)?;
        }
    }

    // Ok now we have our final goal of testing out the changes that we applied.
    // If these changes went awry and actually started to cause the crate to
    // *stop* compiling then we want to back them out and continue to print
    // warnings to the user.
    //
    // If we didn't actually make any changes then we can immediately execute the
    // new rustc, and otherwise we capture the output to hide it in the scenario
    // that we have to back it all out.
    if !fixes.files.is_empty() {
        let mut cmd = Command::new(&rustc);
        args.apply(&mut cmd);
        cmd.arg("--error-format=json");
        let output = cmd.output().context("failed to spawn rustc")?;

        if output.status.success() {
            for (path, file) in fixes.files.iter() {
                Message::Fixing {
                    file: path.clone(),
                    fixes: file.fixes_applied,
                }
                .post()?;
            }
        }

        // If we succeeded then we'll want to commit to the changes we made, if
        // any. If stderr is empty then there's no need for the final exec at
        // the end, we just bail out here.
        if output.status.success() && output.stderr.is_empty() {
            return Ok(true);
        }

        // Otherwise, if our rustc just failed, then that means that we broke the
        // user's code with our changes. Back out everything and fall through
        // below to recompile again.
        if !output.status.success() {
            if env::var_os(BROKEN_CODE_ENV).is_none() {
                for (path, file) in fixes.files.iter() {
                    fs::write(path, &file.original_code)
                        .with_context(|_| format!("failed to write file `{}`", path))?;
                }
            }
            log_failed_fix(&output.stderr)?;
        }
    }

    let mut cmd = Command::new(&rustc);
    args.apply(&mut cmd);
    exit_with(cmd.status().context("failed to spawn rustc")?);
}

#[derive(Default)]
struct FixedCrate {
    files: HashMap<String, FixedFile>,
}

struct FixedFile {
    errors_applying_fixes: Vec<String>,
    fixes_applied: u32,
    original_code: String,
}

fn rustfix_crate(
    lock_addr: &str,
    rustc: &Path,
    filename: &Path,
    args: &FixArgs,
) -> Result<FixedCrate, Error> {
    args.verify_not_preparing_for_enabled_edition()?;

    // First up, we want to make sure that each crate is only checked by one
    // process at a time. If two invocations concurrently check a crate then
    // it's likely to corrupt it.
    //
    // We currently do this by assigning the name on our lock to the manifest
    // directory.
    let dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is missing?");
    let _lock = LockServerClient::lock(&lock_addr.parse()?, dir)?;

    // Next up, this is a bit suspicious, but we *iteratively* execute rustc and
    // collect suggestions to feed to rustfix. Once we hit our limit of times to
    // execute rustc or we appear to be reaching a fixed point we stop running
    // rustc.
    //
    // This is currently done to handle code like:
    //
    //      ::foo::<::Bar>();
    //
    // where there are two fixes to happen here: `crate::foo::<crate::Bar>()`.
    // The spans for these two suggestions are overlapping and its difficult in
    // the compiler to **not** have overlapping spans here. As a result, a naive
    // implementation would feed the two compiler suggestions for the above fix
    // into `rustfix`, but one would be rejected because it overlaps with the
    // other.
    //
    // In this case though, both suggestions are valid and can be automatically
    // applied! To handle this case we execute rustc multiple times, collecting
    // fixes each time we do so. Along the way we discard any suggestions that
    // failed to apply, assuming that they can be fixed the next time we run
    // rustc.
    //
    // Naturally, we want a few protections in place here though to avoid looping
    // forever or otherwise losing data. To that end we have a few termination
    // conditions:
    //
    // * Do this whole process a fixed number of times. In theory we probably
    //   need an infinite number of times to apply fixes, but we're not gonna
    //   sit around waiting for that.
    // * If it looks like a fix genuinely can't be applied we need to bail out.
    //   Detect this when a fix fails to get applied *and* no suggestions
    //   successfully applied to the same file. In that case looks like we
    //   definitely can't make progress, so bail out.
    let mut fixes = FixedCrate::default();
    let mut last_fix_counts = HashMap::new();
    let iterations = env::var("CARGO_FIX_MAX_RETRIES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(4);
    for _ in 0..iterations {
        last_fix_counts.clear();
        for (path, file) in fixes.files.iter_mut() {
            last_fix_counts.insert(path.clone(), file.fixes_applied);
            // We'll generate new errors below.
            file.errors_applying_fixes.clear();
        }
        rustfix_and_fix(&mut fixes, rustc, filename, args)?;
        let mut progress_yet_to_be_made = false;
        for (path, file) in fixes.files.iter_mut() {
            if file.errors_applying_fixes.is_empty() {
                continue;
            }
            // If anything was successfully fixed *and* there's at least one
            // error, then assume the error was spurious and we'll try again on
            // the next iteration.
            if file.fixes_applied != *last_fix_counts.get(path).unwrap_or(&0) {
                progress_yet_to_be_made = true;
            }
        }
        if !progress_yet_to_be_made {
            break;
        }
    }

    // Any errors still remaining at this point need to be reported as probably
    // bugs in Cargo and/or rustfix.
    for (path, file) in fixes.files.iter_mut() {
        for error in file.errors_applying_fixes.drain(..) {
            Message::ReplaceFailed {
                file: path.clone(),
                message: error,
            }
            .post()?;
        }
    }

    Ok(fixes)
}

/// Executes `rustc` to apply one round of suggestions to the crate in question.
///
/// This will fill in the `fixes` map with original code, suggestions applied,
/// and any errors encountered while fixing files.
fn rustfix_and_fix(
    fixes: &mut FixedCrate,
    rustc: &Path,
    filename: &Path,
    args: &FixArgs,
) -> Result<(), Error> {
    // If not empty, filter by these lints.
    // TODO: implement a way to specify this.
    let only = HashSet::new();

    let mut cmd = Command::new(rustc);
    cmd.arg("--error-format=json");
    args.apply(&mut cmd);
    let output = cmd
        .output()
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
        return Ok(());
    }

    let fix_mode = env::var_os("__CARGO_FIX_YOLO")
        .map(|_| rustfix::Filter::Everything)
        .unwrap_or(rustfix::Filter::MachineApplicableOnly);

    // Sift through the output of the compiler to look for JSON messages.
    // indicating fixes that we can apply.
    let stderr = str::from_utf8(&output.stderr).context("failed to parse rustc stderr as UTF-8")?;

    let suggestions = stderr
        .lines()
        .filter(|x| !x.is_empty())
        .inspect(|y| trace!("line: {}", y))
        // Parse each line of stderr, ignoring errors, as they may not all be JSON.
        .filter_map(|line| serde_json::from_str::<Diagnostic>(line).ok())
        // From each diagnostic, try to extract suggestions from rustc.
        .filter_map(|diag| rustfix::collect_suggestions(&diag, &only, fix_mode));

    // Collect suggestions by file so we can apply them one at a time later.
    let mut file_map = HashMap::new();
    let mut num_suggestion = 0;
    for suggestion in suggestions {
        trace!("suggestion");
        // Make sure we've got a file associated with this suggestion and all
        // snippets point to the same file. Right now it's not clear what
        // we would do with multiple files.
        let file_names = suggestion
            .solutions
            .iter()
            .flat_map(|s| s.replacements.iter())
            .map(|r| &r.snippet.file_name);

        let file_name = if let Some(file_name) = file_names.clone().next() {
            file_name.clone()
        } else {
            trace!("rejecting as it has no solutions {:?}", suggestion);
            continue;
        };

        if !file_names.clone().all(|f| f == &file_name) {
            trace!("rejecting as it changes multiple files: {:?}", suggestion);
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
        num_suggestion,
        filename.display(),
    );

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

        // If this file doesn't already exist then we just read the original
        // code, so save it. If the file already exists then the original code
        // doesn't need to be updated as we've just read an interim state with
        // some fixes but perhaps not all.
        let fixed_file = fixes
            .files
            .entry(file.clone())
            .or_insert_with(|| FixedFile {
                errors_applying_fixes: Vec::new(),
                fixes_applied: 0,
                original_code: code.clone(),
            });
        let mut fixed = CodeFix::new(&code);

        // As mentioned above in `rustfix_crate`, we don't immediately warn
        // about suggestions that fail to apply here, and instead we save them
        // off for later processing.
        for suggestion in suggestions.iter().rev() {
            match fixed.apply(suggestion) {
                Ok(()) => fixed_file.fixes_applied += 1,
                Err(e) => fixed_file.errors_applying_fixes.push(e.to_string()),
            }
        }
        let new_code = fixed.finish()?;
        fs::write(&file, new_code).with_context(|_| format!("failed to write file `{}`", file))?;
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

fn log_failed_fix(stderr: &[u8]) -> Result<(), Error> {
    let stderr = str::from_utf8(stderr).context("failed to parse rustc stderr as utf-8")?;

    let diagnostics = stderr
        .lines()
        .filter(|x| !x.is_empty())
        .filter_map(|line| serde_json::from_str::<Diagnostic>(line).ok());
    let mut files = BTreeSet::new();
    let mut errors = Vec::new();
    for diagnostic in diagnostics {
        errors.push(diagnostic.rendered.unwrap_or(diagnostic.message));
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
    Message::FixFailed {
        files,
        krate,
        errors,
    }
    .post()?;

    Ok(())
}

#[derive(Default)]
struct FixArgs {
    file: Option<PathBuf>,
    prepare_for_edition: PrepareFor,
    idioms: bool,
    enabled_edition: Option<String>,
    other: Vec<OsString>,
    primary_package: bool,
}

enum PrepareFor {
    Next,
    Edition(String),
    None,
}

impl Default for PrepareFor {
    fn default() -> PrepareFor {
        PrepareFor::None
    }
}

impl FixArgs {
    fn get() -> FixArgs {
        let mut ret = FixArgs::default();
        for arg in env::args_os().skip(1) {
            let path = PathBuf::from(arg);
            if path.extension().and_then(|s| s.to_str()) == Some("rs") && path.exists() {
                ret.file = Some(path);
                continue;
            }
            if let Some(s) = path.to_str() {
                let prefix = "--edition=";
                if s.starts_with(prefix) {
                    ret.enabled_edition = Some(s[prefix.len()..].to_string());
                    continue;
                }
            }
            ret.other.push(path.into());
        }
        if let Ok(s) = env::var(PREPARE_FOR_ENV) {
            ret.prepare_for_edition = PrepareFor::Edition(s);
        } else if env::var(EDITION_ENV).is_ok() {
            ret.prepare_for_edition = PrepareFor::Next;
        }
        ret.idioms = env::var(IDIOMS_ENV).is_ok();
        ret.primary_package = env::var("CARGO_PRIMARY_PACKAGE").is_ok();
        ret
    }

    fn apply(&self, cmd: &mut Command) {
        if let Some(path) = &self.file {
            cmd.arg(path);
        }
        cmd.args(&self.other).arg("--cap-lints=warn");
        if let Some(edition) = &self.enabled_edition {
            cmd.arg("--edition").arg(edition);
            if self.idioms && self.primary_package && edition == "2018" {
                cmd.arg("-Wrust-2018-idioms");
            }
        }
        if self.primary_package {
            if let Some(edition) = self.prepare_for_edition_resolve() {
                cmd.arg("-W").arg(format!("rust-{}-compatibility", edition));
            }
        }
    }

    /// Verifies that we're not both preparing for an enabled edition and enabling
    /// the edition.
    ///
    /// This indicates that `cargo fix --prepare-for` is being executed out of
    /// order with enabling the edition itself, meaning that we wouldn't
    /// actually be able to fix anything! If it looks like this is happening
    /// then yield an error to the user, indicating that this is happening.
    fn verify_not_preparing_for_enabled_edition(&self) -> CargoResult<()> {
        let edition = match self.prepare_for_edition_resolve() {
            Some(s) => s,
            None => return Ok(()),
        };
        let enabled = match &self.enabled_edition {
            Some(s) => s,
            None => return Ok(()),
        };
        if edition != enabled {
            return Ok(());
        }
        let path = match &self.file {
            Some(s) => s,
            None => return Ok(()),
        };

        Message::EditionAlreadyEnabled {
            file: path.display().to_string(),
            edition: edition.to_string(),
        }
        .post()?;

        process::exit(1);
    }

    fn prepare_for_edition_resolve(&self) -> Option<&str> {
        match &self.prepare_for_edition {
            PrepareFor::Edition(s) => Some(s),
            PrepareFor::Next => Some(self.next_edition()),
            PrepareFor::None => None,
        }
    }

    fn next_edition(&self) -> &str {
        match self.enabled_edition.as_ref().map(|s| &**s) {
            // 2015 -> 2018,
            None | Some("2015") => "2018",

            // This'll probably be wrong in 2020, but that's future Cargo's
            // problem. Eventually though we'll just add more editions here as
            // necessary.
            _ => "2018",
        }
    }
}
