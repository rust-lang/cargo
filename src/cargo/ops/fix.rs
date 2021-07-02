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
//! for rustc by setting `primary_unit_rustc` in the build config. When
//! cargo launches rustc to check a crate, it is actually launching itself.
//! The `FIX_ENV` environment variable is set so that cargo knows it is in
//! fix-proxy-mode.
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
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitStatus};
use std::str;

use anyhow::{bail, Context, Error};
use cargo_util::{paths, ProcessBuilder};
use log::{debug, trace, warn};
use rustfix::diagnostics::Diagnostic;
use rustfix::{self, CodeFix};

use crate::core::compiler::RustcTargetData;
use crate::core::resolver::features::{FeatureOpts, FeatureResolver};
use crate::core::resolver::{HasDevUnits, Resolve, ResolveBehavior};
use crate::core::{Edition, MaybePackage, Workspace};
use crate::ops::{self, CompileOptions};
use crate::util::diagnostic_server::{Message, RustfixDiagnosticServer};
use crate::util::errors::CargoResult;
use crate::util::Config;
use crate::util::{existing_vcs_repo, LockServer, LockServerClient};
use crate::{drop_eprint, drop_eprintln};

const FIX_ENV: &str = "__CARGO_FIX_PLZ";
const BROKEN_CODE_ENV: &str = "__CARGO_FIX_BROKEN_CODE";
const EDITION_ENV: &str = "__CARGO_FIX_EDITION";
const IDIOMS_ENV: &str = "__CARGO_FIX_IDIOMS";

pub struct FixOptions {
    pub edition: bool,
    pub idioms: bool,
    pub compile_opts: CompileOptions,
    pub allow_dirty: bool,
    pub allow_no_vcs: bool,
    pub allow_staged: bool,
    pub broken_code: bool,
}

pub fn fix(ws: &Workspace<'_>, opts: &mut FixOptions) -> CargoResult<()> {
    check_version_control(ws.config(), opts)?;
    if opts.edition {
        check_resolver_change(ws, opts)?;
    }

    // Spin up our lock server, which our subprocesses will use to synchronize fixes.
    let lock_server = LockServer::new()?;
    let mut wrapper = ProcessBuilder::new(env::current_exe()?);
    wrapper.env(FIX_ENV, lock_server.addr().to_string());
    let _started = lock_server.start()?;

    opts.compile_opts.build_config.force_rebuild = true;

    if opts.broken_code {
        wrapper.env(BROKEN_CODE_ENV, "1");
    }

    if opts.edition {
        wrapper.env(EDITION_ENV, "1");
    }
    if opts.idioms {
        wrapper.env(IDIOMS_ENV, "1");
    }

    *opts
        .compile_opts
        .build_config
        .rustfix_diagnostic_server
        .borrow_mut() = Some(RustfixDiagnosticServer::new()?);

    if let Some(server) = opts
        .compile_opts
        .build_config
        .rustfix_diagnostic_server
        .borrow()
        .as_ref()
    {
        server.configure(&mut wrapper);
    }

    let rustc = ws.config().load_global_rustc(Some(ws))?;
    wrapper.arg(&rustc.path);

    // primary crates are compiled using a cargo subprocess to do extra work of applying fixes and
    // repeating build until there are no more changes to be applied
    opts.compile_opts.build_config.primary_unit_rustc = Some(wrapper);

    ops::compile(ws, &opts.compile_opts)?;
    Ok(())
}

fn check_version_control(config: &Config, opts: &FixOptions) -> CargoResult<()> {
    if opts.allow_no_vcs {
        return Ok(());
    }
    if !existing_vcs_repo(config.cwd(), config.cwd()) {
        bail!(
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

    bail!(
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

fn check_resolver_change(ws: &Workspace<'_>, opts: &FixOptions) -> CargoResult<()> {
    let root = ws.root_maybe();
    match root {
        MaybePackage::Package(root_pkg) => {
            if root_pkg.manifest().resolve_behavior().is_some() {
                // If explicitly specified by the user, no need to check.
                return Ok(());
            }
            // Only trigger if updating the root package from 2018.
            let pkgs = opts.compile_opts.spec.get_packages(ws)?;
            if !pkgs.iter().any(|&pkg| pkg == root_pkg) {
                // The root is not being migrated.
                return Ok(());
            }
            if root_pkg.manifest().edition() != Edition::Edition2018 {
                // V1 to V2 only happens on 2018 to 2021.
                return Ok(());
            }
        }
        MaybePackage::Virtual(_vm) => {
            // Virtual workspaces don't have a global edition to set (yet).
            return Ok(());
        }
    }
    // 2018 without `resolver` set must be V1
    assert_eq!(ws.resolve_behavior(), ResolveBehavior::V1);
    let specs = opts.compile_opts.spec.to_package_id_specs(ws)?;
    let target_data = RustcTargetData::new(ws, &opts.compile_opts.build_config.requested_kinds)?;
    // HasDevUnits::No because that may uncover more differences.
    // This is not the same as what `cargo fix` is doing, since it is doing
    // `--all-targets` which includes dev dependencies.
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        &opts.compile_opts.build_config.requested_kinds,
        &opts.compile_opts.cli_features,
        &specs,
        HasDevUnits::No,
        crate::core::resolver::features::ForceAllTargets::No,
    )?;

    let feature_opts = FeatureOpts::new_behavior(ResolveBehavior::V2, HasDevUnits::No);
    let v2_features = FeatureResolver::resolve(
        ws,
        &target_data,
        &ws_resolve.targeted_resolve,
        &ws_resolve.pkg_set,
        &opts.compile_opts.cli_features,
        &specs,
        &opts.compile_opts.build_config.requested_kinds,
        feature_opts,
    )?;

    let differences = v2_features.compare_legacy(&ws_resolve.resolved_features);
    if differences.is_empty() {
        // Nothing is different, nothing to report.
        return Ok(());
    }
    let config = ws.config();
    config.shell().note(
        "Switching to Edition 2021 will enable the use of the version 2 feature resolver in Cargo.",
    )?;
    drop_eprintln!(
        config,
        "This may cause some dependencies to be built with fewer features enabled than previously."
    );
    drop_eprintln!(
        config,
        "More information about the resolver changes may be found \
         at https://doc.rust-lang.org/nightly/edition-guide/rust-2021/default-cargo-resolver.html"
    );
    drop_eprintln!(
        config,
        "When building the following dependencies, \
         the given features will no longer be used:\n"
    );
    for ((pkg_id, for_host), removed) in differences {
        drop_eprint!(config, "  {}", pkg_id);
        if for_host {
            drop_eprint!(config, " (as host dependency)");
        }
        drop_eprint!(config, ": ");
        let joined: Vec<_> = removed.iter().map(|s| s.as_str()).collect();
        drop_eprintln!(config, "{}", joined.join(", "));
    }
    drop_eprint!(config, "\n");
    report_maybe_diesel(config, &ws_resolve.targeted_resolve)?;
    Ok(())
}

fn report_maybe_diesel(config: &Config, resolve: &Resolve) -> CargoResult<()> {
    if resolve
        .iter()
        .any(|pid| pid.name() == "diesel" && pid.version().major == 1)
        && resolve.iter().any(|pid| pid.name() == "diesel_migrations")
    {
        config.shell().note(
            "\
This project appears to use both diesel and diesel_migrations. These packages have
a known issue where the build may fail due to the version 2 resolver preventing
feature unification between those two packages. See
<https://github.com/rust-lang/cargo/issues/9450> for some potential workarounds.
",
        )?;
    }
    Ok(())
}

/// Entry point for `cargo` running as a proxy for `rustc`.
///
/// This is called every time `cargo` is run to check if it is in proxy mode.
///
/// Returns `false` if `fix` is not being run (not in proxy mode). Returns
/// `true` if in `fix` proxy mode, and the fix was complete without any
/// warnings or errors. If there are warnings or errors, this does not return,
/// and the process exits with the corresponding `rustc` exit code.
pub fn fix_maybe_exec_rustc(config: &Config) -> CargoResult<bool> {
    let lock_addr = match env::var(FIX_ENV) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    let args = FixArgs::get()?;
    trace!("cargo-fix as rustc got file {:?}", args.file);

    let workspace_rustc = std::env::var("RUSTC_WORKSPACE_WRAPPER")
        .map(PathBuf::from)
        .ok();
    let rustc = ProcessBuilder::new(&args.rustc).wrapped(workspace_rustc.as_ref());

    trace!("start rustfixing {:?}", args.file);
    let fixes = rustfix_crate(&lock_addr, &rustc, &args.file, &args, config)?;

    // Ok now we have our final goal of testing out the changes that we applied.
    // If these changes went awry and actually started to cause the crate to
    // *stop* compiling then we want to back them out and continue to print
    // warnings to the user.
    //
    // If we didn't actually make any changes then we can immediately execute the
    // new rustc, and otherwise we capture the output to hide it in the scenario
    // that we have to back it all out.
    if !fixes.files.is_empty() {
        let mut cmd = rustc.build_command();
        args.apply(&mut cmd, config);
        cmd.arg("--error-format=json");
        let output = cmd.output().context("failed to spawn rustc")?;

        if output.status.success() {
            for (path, file) in fixes.files.iter() {
                Message::Fixed {
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
                    paths::write(path, &file.original_code)?;
                }
            }
            log_failed_fix(&output.stderr)?;
        }
    }

    // This final fall-through handles multiple cases;
    // - If the fix failed, show the original warnings and suggestions.
    // - If `--broken-code`, show the error messages.
    // - If the fix succeeded, show any remaining warnings.
    let mut cmd = rustc.build_command();
    args.apply(&mut cmd, config);
    for arg in args.format_args {
        // Add any json/error format arguments that Cargo wants. This allows
        // things like colored output to work correctly.
        cmd.arg(arg);
    }
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

/// Attempts to apply fixes to a single crate.
///
/// This runs `rustc` (possibly multiple times) to gather suggestions from the
/// compiler and applies them to the files on disk.
fn rustfix_crate(
    lock_addr: &str,
    rustc: &ProcessBuilder,
    filename: &Path,
    args: &FixArgs,
    config: &Config,
) -> Result<FixedCrate, Error> {
    args.check_edition_and_send_status(config)?;

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
        rustfix_and_fix(&mut fixes, rustc, filename, args, config)?;
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
    rustc: &ProcessBuilder,
    filename: &Path,
    args: &FixArgs,
    config: &Config,
) -> Result<(), Error> {
    // If not empty, filter by these lints.
    // TODO: implement a way to specify this.
    let only = HashSet::new();

    let mut cmd = rustc.build_command();
    cmd.arg("--error-format=json");
    args.apply(&mut cmd, config);
    let output = cmd.output().with_context(|| {
        format!(
            "failed to execute `{}`",
            rustc.get_program().to_string_lossy()
        )
    })?;

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
        paths::write(&file, new_code)?;
    }

    Ok(())
}

fn exit_with(status: ExitStatus) -> ! {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::prelude::*;
        if let Some(signal) = status.signal() {
            drop(writeln!(
                std::io::stderr().lock(),
                "child failed with signal `{}`",
                signal
            ));
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

/// Various command-line options and settings used when `cargo` is running as
/// a proxy for `rustc` during the fix operation.
struct FixArgs {
    /// This is the `.rs` file that is being fixed.
    file: PathBuf,
    /// If `--edition` is used to migrate to the next edition, this is the
    /// edition we are migrating towards.
    prepare_for_edition: Option<Edition>,
    /// `true` if `--edition-idioms` is enabled.
    idioms: bool,
    /// The current edition.
    ///
    /// `None` if on 2015.
    enabled_edition: Option<Edition>,
    /// Other command-line arguments not reflected by other fields in
    /// `FixArgs`.
    other: Vec<OsString>,
    /// Path to the `rustc` executable.
    rustc: PathBuf,
    /// Console output flags (`--error-format`, `--json`, etc.).
    ///
    /// The normal fix procedure always uses `--json`, so it overrides what
    /// Cargo normally passes when applying fixes. When displaying warnings or
    /// errors, it will use these flags.
    format_args: Vec<String>,
}

impl FixArgs {
    fn get() -> Result<FixArgs, Error> {
        let rustc = env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("expected rustc as first argument"))?;
        let mut file = None;
        let mut enabled_edition = None;
        let mut other = Vec::new();
        let mut format_args = Vec::new();

        for arg in env::args_os().skip(2) {
            let path = PathBuf::from(arg);
            if path.extension().and_then(|s| s.to_str()) == Some("rs") && path.exists() {
                file = Some(path);
                continue;
            }
            if let Some(s) = path.to_str() {
                if let Some(edition) = s.strip_prefix("--edition=") {
                    enabled_edition = Some(edition.parse()?);
                    continue;
                }
                if s.starts_with("--error-format=") || s.starts_with("--json=") {
                    // Cargo may add error-format in some cases, but `cargo
                    // fix` wants to add its own.
                    format_args.push(s.to_string());
                    continue;
                }
            }
            other.push(path.into());
        }
        let file = file.ok_or_else(|| anyhow::anyhow!("could not find .rs file in rustc args"))?;
        let idioms = env::var(IDIOMS_ENV).is_ok();

        let prepare_for_edition = env::var(EDITION_ENV).ok().map(|_| {
            enabled_edition
                .unwrap_or(Edition::Edition2015)
                .saturating_next()
        });

        Ok(FixArgs {
            file,
            prepare_for_edition,
            idioms,
            enabled_edition,
            other,
            rustc,
            format_args,
        })
    }

    fn apply(&self, cmd: &mut Command, config: &Config) {
        cmd.arg(&self.file);
        cmd.args(&self.other).arg("--cap-lints=warn");
        if let Some(edition) = self.enabled_edition {
            cmd.arg("--edition").arg(edition.to_string());
            if self.idioms && edition.supports_idiom_lint() {
                cmd.arg(format!("-Wrust-{}-idioms", edition));
            }
        }

        if let Some(edition) = self.prepare_for_edition {
            if edition.supports_compat_lint() {
                if config.nightly_features_allowed {
                    cmd.arg("--force-warns")
                        .arg(format!("rust-{}-compatibility", edition))
                        .arg("-Zunstable-options");
                } else {
                    cmd.arg("-W").arg(format!("rust-{}-compatibility", edition));
                }
            }
        }
    }

    /// Validates the edition, and sends a message indicating what is being
    /// done.
    fn check_edition_and_send_status(&self, config: &Config) -> CargoResult<()> {
        let to_edition = match self.prepare_for_edition {
            Some(s) => s,
            None => {
                return Message::Fixing {
                    file: self.file.display().to_string(),
                }
                .post();
            }
        };
        // Unfortunately determining which cargo targets are being built
        // isn't easy, and each target can be a different edition. The
        // cargo-as-rustc fix wrapper doesn't know anything about the
        // workspace, so it can't check for the `cargo-features` unstable
        // opt-in. As a compromise, this just restricts to the nightly
        // toolchain.
        //
        // Unfortunately this results in a pretty poor error message when
        // multiple jobs run in parallel (the error appears multiple
        // times). Hopefully this doesn't happen often in practice.
        if !to_edition.is_stable() && !config.nightly_features_allowed {
            bail!(
                "cannot migrate {} to edition {to_edition}\n\
                 Edition {to_edition} is unstable and not allowed in this release, \
                 consider trying the nightly release channel.",
                self.file.display(),
                to_edition = to_edition
            );
        }
        let from_edition = self.enabled_edition.unwrap_or(Edition::Edition2015);
        if from_edition == to_edition {
            Message::EditionAlreadyEnabled {
                file: self.file.display().to_string(),
                edition: to_edition,
            }
            .post()
        } else {
            Message::Migrating {
                file: self.file.display().to_string(),
                from_edition,
                to_edition,
            }
            .post()
        }
    }
}
