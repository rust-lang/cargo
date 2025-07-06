//! High-level overview of how `fix` works:
//!
//! The main goal is to run `cargo check` to get rustc to emit JSON
//! diagnostics with suggested fixes that can be applied to the files on the
//! filesystem, and validate that those changes didn't break anything.
//!
//! Cargo begins by launching a [`LockServer`] thread in the background to
//! listen for network connections to coordinate locking when multiple targets
//! are built simultaneously. It ensures each package has only one fix running
//! at once.
//!
//! The [`RustfixDiagnosticServer`] is launched in a background thread (in
//! `JobQueue`) to listen for network connections to coordinate displaying
//! messages to the user on the console (so that multiple processes don't try
//! to print at the same time).
//!
//! Cargo begins a normal `cargo check` operation with itself set as a proxy
//! for rustc by setting `BuildConfig::primary_unit_rustc` in the build config. When
//! cargo launches rustc to check a crate, it is actually launching itself.
//! The `FIX_ENV_INTERNAL` environment variable is set to the value of the [`LockServer`]'s
//! address so that cargo knows it is in fix-proxy-mode.
//!
//! Each proxied cargo-as-rustc detects it is in fix-proxy-mode (via `FIX_ENV_INTERNAL`
//! environment variable in `main`) and does the following:
//!
//! - Acquire a lock from the [`LockServer`] from the master cargo process.
//! - Launches the real rustc ([`rustfix_and_fix`]), looking at the JSON output
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

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, ExitStatus, Output};
use std::{env, fs, str};

use anyhow::{Context as _, bail};
use cargo_util::{ProcessBuilder, exit_status_to_string, is_simple_exit_code, paths};
use cargo_util_schemas::manifest::TomlManifest;
use rustfix::CodeFix;
use rustfix::diagnostics::Diagnostic;
use semver::Version;
use tracing::{debug, trace, warn};

pub use self::fix_edition::fix_edition;
use crate::core::PackageIdSpecQuery as _;
use crate::core::compiler::CompileKind;
use crate::core::compiler::RustcTargetData;
use crate::core::resolver::features::{DiffMap, FeatureOpts, FeatureResolver, FeaturesFor};
use crate::core::resolver::{HasDevUnits, Resolve, ResolveBehavior};
use crate::core::{Edition, MaybePackage, Package, PackageId, Workspace};
use crate::ops::resolve::WorkspaceResolve;
use crate::ops::{self, CompileOptions};
use crate::util::GlobalContext;
use crate::util::diagnostic_server::{Message, RustfixDiagnosticServer};
use crate::util::errors::CargoResult;
use crate::util::toml_mut::manifest::LocalManifest;
use crate::util::{LockServer, LockServerClient, existing_vcs_repo};
use crate::{drop_eprint, drop_eprintln};

mod fix_edition;

/// **Internal only.**
/// Indicates Cargo is in fix-proxy-mode if presents.
/// The value of it is the socket address of the [`LockServer`] being used.
/// See the [module-level documentation](mod@super::fix) for more.
const FIX_ENV_INTERNAL: &str = "__CARGO_FIX_PLZ";
/// **Internal only.**
/// For passing [`FixOptions::broken_code`] through to cargo running in proxy mode.
const BROKEN_CODE_ENV_INTERNAL: &str = "__CARGO_FIX_BROKEN_CODE";
/// **Internal only.**
/// For passing [`FixOptions::edition`] through to cargo running in proxy mode.
const EDITION_ENV_INTERNAL: &str = "__CARGO_FIX_EDITION";
/// **Internal only.**
/// For passing [`FixOptions::idioms`] through to cargo running in proxy mode.
const IDIOMS_ENV_INTERNAL: &str = "__CARGO_FIX_IDIOMS";
/// **Internal only.**
/// The sysroot path.
///
/// This is for preventing `cargo fix` from fixing rust std/core libs. See
///
/// * <https://github.com/rust-lang/cargo/issues/9857>
/// * <https://github.com/rust-lang/rust/issues/88514#issuecomment-2043469384>
const SYSROOT_INTERNAL: &str = "__CARGO_FIX_RUST_SRC";

pub struct FixOptions {
    pub edition: Option<EditionFixMode>,
    pub idioms: bool,
    pub compile_opts: CompileOptions,
    pub allow_dirty: bool,
    pub allow_no_vcs: bool,
    pub allow_staged: bool,
    pub broken_code: bool,
    pub requested_lockfile_path: Option<PathBuf>,
}

/// The behavior of `--edition` migration.
#[derive(Clone, Copy)]
pub enum EditionFixMode {
    /// Migrates the package from the current edition to the next.
    ///
    /// This is the normal (stable) behavior of `--edition`.
    NextRelative,
    /// Migrates to a specific edition.
    ///
    /// This is used by `-Zfix-edition` to force a specific edition like
    /// `future`, which does not have a relative value.
    OverrideSpecific(Edition),
}

impl EditionFixMode {
    /// Returns the edition to use for the given current edition.
    pub fn next_edition(&self, current_edition: Edition) -> Edition {
        match self {
            EditionFixMode::NextRelative => current_edition.saturating_next(),
            EditionFixMode::OverrideSpecific(edition) => *edition,
        }
    }

    /// Serializes to a string.
    fn to_string(&self) -> String {
        match self {
            EditionFixMode::NextRelative => "1".to_string(),
            EditionFixMode::OverrideSpecific(edition) => edition.to_string(),
        }
    }

    /// Deserializes from the given string.
    fn from_str(s: &str) -> EditionFixMode {
        match s {
            "1" => EditionFixMode::NextRelative,
            edition => EditionFixMode::OverrideSpecific(edition.parse().unwrap()),
        }
    }
}

pub fn fix(
    gctx: &GlobalContext,
    original_ws: &Workspace<'_>,
    opts: &mut FixOptions,
) -> CargoResult<()> {
    check_version_control(gctx, opts)?;

    let mut target_data =
        RustcTargetData::new(original_ws, &opts.compile_opts.build_config.requested_kinds)?;
    if let Some(edition_mode) = opts.edition {
        let specs = opts.compile_opts.spec.to_package_id_specs(&original_ws)?;
        let members: Vec<&Package> = original_ws
            .members()
            .filter(|m| specs.iter().any(|spec| spec.matches(m.package_id())))
            .collect();
        migrate_manifests(original_ws, &members, edition_mode)?;

        check_resolver_change(&original_ws, &mut target_data, opts)?;
    }
    let ws = original_ws.reload(gctx)?;

    // Spin up our lock server, which our subprocesses will use to synchronize fixes.
    let lock_server = LockServer::new()?;
    let mut wrapper = ProcessBuilder::new(env::current_exe()?);
    wrapper.env(FIX_ENV_INTERNAL, lock_server.addr().to_string());
    let _started = lock_server.start()?;

    opts.compile_opts.build_config.force_rebuild = true;

    if opts.broken_code {
        wrapper.env(BROKEN_CODE_ENV_INTERNAL, "1");
    }

    if let Some(mode) = &opts.edition {
        wrapper.env(EDITION_ENV_INTERNAL, mode.to_string());
    }
    if opts.idioms {
        wrapper.env(IDIOMS_ENV_INTERNAL, "1");
    }

    let sysroot = &target_data.info(CompileKind::Host).sysroot;
    if sysroot.is_dir() {
        wrapper.env(SYSROOT_INTERNAL, sysroot);
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

    let rustc = ws.gctx().load_global_rustc(Some(&ws))?;
    wrapper.arg(&rustc.path);
    // This is calling rustc in cargo fix-proxy-mode, so it also need to retry.
    // The argfile handling are located at `FixArgs::from_args`.
    wrapper.retry_with_argfile(true);

    // primary crates are compiled using a cargo subprocess to do extra work of applying fixes and
    // repeating build until there are no more changes to be applied
    opts.compile_opts.build_config.primary_unit_rustc = Some(wrapper);

    ops::compile(&ws, &opts.compile_opts)?;
    Ok(())
}

fn check_version_control(gctx: &GlobalContext, opts: &FixOptions) -> CargoResult<()> {
    if opts.allow_no_vcs {
        return Ok(());
    }
    if !existing_vcs_repo(gctx.cwd(), gctx.cwd()) {
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
    if let Ok(repo) = git2::Repository::discover(gctx.cwd()) {
        let mut repo_opts = git2::StatusOptions::new();
        repo_opts.include_ignored(false);
        repo_opts.include_untracked(true);
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
         like to suppress this error pass `--allow-dirty`, \
         or commit the changes to these files:\n\
         \n\
         {}\n\
         ",
        files_list
    );
}

fn migrate_manifests(
    ws: &Workspace<'_>,
    pkgs: &[&Package],
    edition_mode: EditionFixMode,
) -> CargoResult<()> {
    // HACK: Duplicate workspace migration logic between virtual manifests and real manifests to
    // reduce multiple Migrating messages being reported for the same file to the user
    if matches!(ws.root_maybe(), MaybePackage::Virtual(_)) {
        // Warning: workspaces do not have an edition so this should only include changes needed by
        // packages that preserve the behavior of the workspace on all editions
        let highest_edition = pkgs
            .iter()
            .map(|p| p.manifest().edition())
            .max()
            .unwrap_or_default();
        let prepare_for_edition = edition_mode.next_edition(highest_edition);
        if highest_edition == prepare_for_edition
            || (!prepare_for_edition.is_stable() && !ws.gctx().nightly_features_allowed)
        {
            //
        } else {
            let mut manifest_mut = LocalManifest::try_new(ws.root_manifest())?;
            let document = &mut manifest_mut.data;
            let mut fixes = 0;

            if Edition::Edition2024 <= prepare_for_edition {
                let root = document.as_table_mut();

                if let Some(workspace) = root
                    .get_mut("workspace")
                    .and_then(|t| t.as_table_like_mut())
                {
                    // strictly speaking, the edition doesn't apply to this table but it should be safe
                    // enough
                    fixes += rename_dep_fields_2024(workspace, "dependencies");
                }
            }

            if 0 < fixes {
                // HACK: As workspace migration is a special case, only report it if something
                // happened
                let file = ws.root_manifest();
                let file = file.strip_prefix(ws.root()).unwrap_or(file);
                let file = file.display();
                ws.gctx().shell().status(
                    "Migrating",
                    format!("{file} from {highest_edition} edition to {prepare_for_edition}"),
                )?;

                let verb = if fixes == 1 { "fix" } else { "fixes" };
                let msg = format!("{file} ({fixes} {verb})");
                ws.gctx().shell().status("Fixed", msg)?;

                manifest_mut.write()?;
            }
        }
    }

    for pkg in pkgs {
        let existing_edition = pkg.manifest().edition();
        let prepare_for_edition = edition_mode.next_edition(existing_edition);
        if existing_edition == prepare_for_edition
            || (!prepare_for_edition.is_stable() && !ws.gctx().nightly_features_allowed)
        {
            continue;
        }
        let file = pkg.manifest_path();
        let file = file.strip_prefix(ws.root()).unwrap_or(file);
        let file = file.display();
        ws.gctx().shell().status(
            "Migrating",
            format!("{file} from {existing_edition} edition to {prepare_for_edition}"),
        )?;

        let mut manifest_mut = LocalManifest::try_new(pkg.manifest_path())?;
        let document = &mut manifest_mut.data;
        let mut fixes = 0;

        let ws_original_toml = match ws.root_maybe() {
            MaybePackage::Package(package) => package.manifest().original_toml(),
            MaybePackage::Virtual(manifest) => manifest.original_toml(),
        };
        if Edition::Edition2024 <= prepare_for_edition {
            let root = document.as_table_mut();

            if let Some(workspace) = root
                .get_mut("workspace")
                .and_then(|t| t.as_table_like_mut())
            {
                // strictly speaking, the edition doesn't apply to this table but it should be safe
                // enough
                fixes += rename_dep_fields_2024(workspace, "dependencies");
            }

            fixes += rename_table(root, "project", "package");
            if let Some(target) = root.get_mut("lib").and_then(|t| t.as_table_like_mut()) {
                fixes += rename_target_fields_2024(target);
            }
            fixes += rename_array_of_target_fields_2024(root, "bin");
            fixes += rename_array_of_target_fields_2024(root, "example");
            fixes += rename_array_of_target_fields_2024(root, "test");
            fixes += rename_array_of_target_fields_2024(root, "bench");
            fixes += rename_dep_fields_2024(root, "dependencies");
            fixes += remove_ignored_default_features_2024(root, "dependencies", ws_original_toml);
            fixes += rename_table(root, "dev_dependencies", "dev-dependencies");
            fixes += rename_dep_fields_2024(root, "dev-dependencies");
            fixes +=
                remove_ignored_default_features_2024(root, "dev-dependencies", ws_original_toml);
            fixes += rename_table(root, "build_dependencies", "build-dependencies");
            fixes += rename_dep_fields_2024(root, "build-dependencies");
            fixes +=
                remove_ignored_default_features_2024(root, "build-dependencies", ws_original_toml);
            for target in root
                .get_mut("target")
                .and_then(|t| t.as_table_like_mut())
                .iter_mut()
                .flat_map(|t| t.iter_mut())
                .filter_map(|(_k, t)| t.as_table_like_mut())
            {
                fixes += rename_dep_fields_2024(target, "dependencies");
                fixes +=
                    remove_ignored_default_features_2024(target, "dependencies", ws_original_toml);
                fixes += rename_table(target, "dev_dependencies", "dev-dependencies");
                fixes += rename_dep_fields_2024(target, "dev-dependencies");
                fixes += remove_ignored_default_features_2024(
                    target,
                    "dev-dependencies",
                    ws_original_toml,
                );
                fixes += rename_table(target, "build_dependencies", "build-dependencies");
                fixes += rename_dep_fields_2024(target, "build-dependencies");
                fixes += remove_ignored_default_features_2024(
                    target,
                    "build-dependencies",
                    ws_original_toml,
                );
            }
        }

        if 0 < fixes {
            let verb = if fixes == 1 { "fix" } else { "fixes" };
            let msg = format!("{file} ({fixes} {verb})");
            ws.gctx().shell().status("Fixed", msg)?;

            manifest_mut.write()?;
        }
    }

    Ok(())
}

fn rename_dep_fields_2024(parent: &mut dyn toml_edit::TableLike, dep_kind: &str) -> usize {
    let mut fixes = 0;
    for target in parent
        .get_mut(dep_kind)
        .and_then(|t| t.as_table_like_mut())
        .iter_mut()
        .flat_map(|t| t.iter_mut())
        .filter_map(|(_k, t)| t.as_table_like_mut())
    {
        fixes += rename_table(target, "default_features", "default-features");
    }
    fixes
}

fn remove_ignored_default_features_2024(
    parent: &mut dyn toml_edit::TableLike,
    dep_kind: &str,
    ws_original_toml: &TomlManifest,
) -> usize {
    let mut fixes = 0;
    for (name_in_toml, target) in parent
        .get_mut(dep_kind)
        .and_then(|t| t.as_table_like_mut())
        .iter_mut()
        .flat_map(|t| t.iter_mut())
        .filter_map(|(k, t)| t.as_table_like_mut().map(|t| (k, t)))
    {
        let name_in_toml: &str = &name_in_toml;
        let ws_deps = ws_original_toml
            .workspace
            .as_ref()
            .and_then(|ws| ws.dependencies.as_ref());
        if let Some(ws_dep) = ws_deps.and_then(|ws_deps| ws_deps.get(name_in_toml)) {
            if ws_dep.default_features() == Some(false) {
                continue;
            }
        }
        if target
            .get("workspace")
            .and_then(|i| i.as_value())
            .and_then(|i| i.as_bool())
            == Some(true)
            && target
                .get("default-features")
                .and_then(|i| i.as_value())
                .and_then(|i| i.as_bool())
                == Some(false)
        {
            target.remove("default-features");
            fixes += 1;
        }
    }
    fixes
}

fn rename_array_of_target_fields_2024(root: &mut dyn toml_edit::TableLike, kind: &str) -> usize {
    let mut fixes = 0;
    for target in root
        .get_mut(kind)
        .and_then(|t| t.as_array_of_tables_mut())
        .iter_mut()
        .flat_map(|t| t.iter_mut())
    {
        fixes += rename_target_fields_2024(target);
    }
    fixes
}

fn rename_target_fields_2024(target: &mut dyn toml_edit::TableLike) -> usize {
    let mut fixes = 0;
    fixes += rename_table(target, "crate_type", "crate-type");
    fixes += rename_table(target, "proc_macro", "proc-macro");
    fixes
}

fn rename_table(parent: &mut dyn toml_edit::TableLike, old: &str, new: &str) -> usize {
    let Some(old_key) = parent.key(old).cloned() else {
        return 0;
    };

    let project = parent.remove(old).expect("returned early");
    if !parent.contains_key(new) {
        parent.insert(new, project);
        let mut new_key = parent.key_mut(new).expect("just inserted");
        *new_key.dotted_decor_mut() = old_key.dotted_decor().clone();
        *new_key.leaf_decor_mut() = old_key.leaf_decor().clone();
    }
    1
}

fn check_resolver_change<'gctx>(
    ws: &Workspace<'gctx>,
    target_data: &mut RustcTargetData<'gctx>,
    opts: &FixOptions,
) -> CargoResult<()> {
    let root = ws.root_maybe();
    match root {
        MaybePackage::Package(root_pkg) => {
            if root_pkg.manifest().resolve_behavior().is_some() {
                // If explicitly specified by the user, no need to check.
                return Ok(());
            }
            // Only trigger if updating the root package from 2018.
            let pkgs = opts.compile_opts.spec.get_packages(ws)?;
            if !pkgs.contains(&root_pkg) {
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
    let mut resolve_differences = |has_dev_units| -> CargoResult<(WorkspaceResolve<'_>, DiffMap)> {
        let dry_run = false;
        let ws_resolve = ops::resolve_ws_with_opts(
            ws,
            target_data,
            &opts.compile_opts.build_config.requested_kinds,
            &opts.compile_opts.cli_features,
            &specs,
            has_dev_units,
            crate::core::resolver::features::ForceAllTargets::No,
            dry_run,
        )?;

        let feature_opts = FeatureOpts::new_behavior(ResolveBehavior::V2, has_dev_units);
        let v2_features = FeatureResolver::resolve(
            ws,
            target_data,
            &ws_resolve.targeted_resolve,
            &ws_resolve.pkg_set,
            &opts.compile_opts.cli_features,
            &specs,
            &opts.compile_opts.build_config.requested_kinds,
            feature_opts,
        )?;

        if ws_resolve.specs_and_features.len() != 1 {
            bail!(r#"cannot fix edition when using `feature-unification = "package"`."#);
        }
        let resolved_features = &ws_resolve
            .specs_and_features
            .first()
            .expect("We've already checked that there is exactly one.")
            .resolved_features;
        let diffs = v2_features.compare_legacy(resolved_features);
        Ok((ws_resolve, diffs))
    };
    let (_, without_dev_diffs) = resolve_differences(HasDevUnits::No)?;
    let (ws_resolve, mut with_dev_diffs) = resolve_differences(HasDevUnits::Yes)?;
    if without_dev_diffs.is_empty() && with_dev_diffs.is_empty() {
        // Nothing is different, nothing to report.
        return Ok(());
    }
    // Only display unique changes with dev-dependencies.
    with_dev_diffs.retain(|k, vals| without_dev_diffs.get(k) != Some(vals));
    let gctx = ws.gctx();
    gctx.shell().note(
        "Switching to Edition 2021 will enable the use of the version 2 feature resolver in Cargo.",
    )?;
    drop_eprintln!(
        gctx,
        "This may cause some dependencies to be built with fewer features enabled than previously."
    );
    drop_eprintln!(
        gctx,
        "More information about the resolver changes may be found \
         at https://doc.rust-lang.org/nightly/edition-guide/rust-2021/default-cargo-resolver.html"
    );
    drop_eprintln!(
        gctx,
        "When building the following dependencies, \
         the given features will no longer be used:\n"
    );
    let show_diffs = |differences: DiffMap| {
        for ((pkg_id, features_for), removed) in differences {
            drop_eprint!(gctx, "  {}", pkg_id);
            if let FeaturesFor::HostDep = features_for {
                drop_eprint!(gctx, " (as host dependency)");
            }
            drop_eprint!(gctx, " removed features: ");
            let joined: Vec<_> = removed.iter().map(|s| s.as_str()).collect();
            drop_eprintln!(gctx, "{}", joined.join(", "));
        }
        drop_eprint!(gctx, "\n");
    };
    if !without_dev_diffs.is_empty() {
        show_diffs(without_dev_diffs);
    }
    if !with_dev_diffs.is_empty() {
        drop_eprintln!(
            gctx,
            "The following differences only apply when building with dev-dependencies:\n"
        );
        show_diffs(with_dev_diffs);
    }
    report_maybe_diesel(gctx, &ws_resolve.targeted_resolve)?;
    Ok(())
}

fn report_maybe_diesel(gctx: &GlobalContext, resolve: &Resolve) -> CargoResult<()> {
    fn is_broken_diesel(pid: PackageId) -> bool {
        pid.name() == "diesel" && pid.version() < &Version::new(1, 4, 8)
    }

    fn is_broken_diesel_migration(pid: PackageId) -> bool {
        pid.name() == "diesel_migrations" && pid.version().major <= 1
    }

    if resolve.iter().any(is_broken_diesel) && resolve.iter().any(is_broken_diesel_migration) {
        gctx.shell().note(
            "\
This project appears to use both diesel and diesel_migrations. These packages have
a known issue where the build may fail due to the version 2 resolver preventing
feature unification between those two packages. Please update to at least diesel 1.4.8
to prevent this issue from happening.
",
        )?;
    }
    Ok(())
}

/// Provide the lock address when running in proxy mode
///
/// Returns `None` if `fix` is not being run (not in proxy mode). Returns
/// `Some(...)` if in `fix` proxy mode
pub fn fix_get_proxy_lock_addr() -> Option<String> {
    // ALLOWED: For the internal mechanism of `cargo fix` only.
    // Shouldn't be set directly by anyone.
    #[allow(clippy::disallowed_methods)]
    env::var(FIX_ENV_INTERNAL).ok()
}

/// Entry point for `cargo` running as a proxy for `rustc`.
///
/// This is called every time `cargo` is run to check if it is in proxy mode.
///
/// If there are warnings or errors, this does not return,
/// and the process exits with the corresponding `rustc` exit code.
///
/// See [`fix_get_proxy_lock_addr`]
pub fn fix_exec_rustc(gctx: &GlobalContext, lock_addr: &str) -> CargoResult<()> {
    let args = FixArgs::get()?;
    trace!("cargo-fix as rustc got file {:?}", args.file);

    let workspace_rustc = gctx
        .get_env("RUSTC_WORKSPACE_WRAPPER")
        .map(PathBuf::from)
        .ok();
    let mut rustc = ProcessBuilder::new(&args.rustc).wrapped(workspace_rustc.as_ref());
    rustc.retry_with_argfile(true);
    rustc.env_remove(FIX_ENV_INTERNAL);
    args.apply(&mut rustc);
    // Removes `FD_CLOEXEC` set by `jobserver::Client` to ensure that the
    // compiler can access the jobserver.
    if let Some(client) = gctx.jobserver_from_env() {
        rustc.inherit_jobserver(client);
    }

    trace!("start rustfixing {:?}", args.file);
    let fixes = rustfix_crate(&lock_addr, &rustc, &args.file, &args, gctx)?;

    if fixes.last_output.status.success() {
        for (path, file) in fixes.files.iter() {
            Message::Fixed {
                file: path.clone(),
                fixes: file.fixes_applied,
            }
            .post(gctx)?;
        }
        // Display any remaining diagnostics.
        emit_output(&fixes.last_output)?;
        return Ok(());
    }

    let allow_broken_code = gctx.get_env_os(BROKEN_CODE_ENV_INTERNAL).is_some();

    // There was an error running rustc during the last run.
    //
    // Back out all of the changes unless --broken-code was used.
    if !allow_broken_code {
        for (path, file) in fixes.files.iter() {
            debug!("reverting {:?} due to errors", path);
            paths::write(path, &file.original_code)?;
        }
    }

    // If there were any fixes, let the user know that there was a failure
    // attempting to apply them, and to ask for a bug report.
    //
    // FIXME: The error message here is not correct with --broken-code.
    //        https://github.com/rust-lang/cargo/issues/10955
    if fixes.files.is_empty() {
        // No fixes were available. Display whatever errors happened.
        emit_output(&fixes.last_output)?;
        exit_with(fixes.last_output.status);
    } else {
        let krate = {
            let mut iter = rustc.get_args();
            let mut krate = None;
            while let Some(arg) = iter.next() {
                if arg == "--crate-name" {
                    krate = iter.next().and_then(|s| s.to_owned().into_string().ok());
                }
            }
            krate
        };
        log_failed_fix(
            gctx,
            krate,
            &fixes.last_output.stderr,
            fixes.last_output.status,
        )?;
        // Display the diagnostics that appeared at the start, before the
        // fixes failed. This can help with diagnosing which suggestions
        // caused the failure.
        emit_output(&fixes.first_output)?;
        // Exit with whatever exit code we initially started with. `cargo fix`
        // treats this as a warning, and shouldn't return a failure code
        // unless the code didn't compile in the first place.
        exit_with(fixes.first_output.status);
    }
}

fn emit_output(output: &Output) -> CargoResult<()> {
    // Unfortunately if there is output on stdout, this does not preserve the
    // order of output relative to stderr. In practice, rustc should never
    // print to stdout unless some proc-macro does it.
    std::io::stderr().write_all(&output.stderr)?;
    std::io::stdout().write_all(&output.stdout)?;
    Ok(())
}

struct FixedCrate {
    /// Map of file path to some information about modifications made to that file.
    files: HashMap<String, FixedFile>,
    /// The output from rustc from the first time it was called.
    ///
    /// This is needed when fixes fail to apply, so that it can display the
    /// original diagnostics to the user which can help with diagnosing which
    /// suggestions caused the failure.
    first_output: Output,
    /// The output from rustc from the last time it was called.
    ///
    /// This will be displayed to the user to show any remaining diagnostics
    /// or errors.
    last_output: Output,
}

#[derive(Debug)]
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
    gctx: &GlobalContext,
) -> CargoResult<FixedCrate> {
    // First up, we want to make sure that each crate is only checked by one
    // process at a time. If two invocations concurrently check a crate then
    // it's likely to corrupt it.
    //
    // Historically this used per-source-file locking, then per-package
    // locking. It now uses a single, global lock as some users do things like
    // #[path] or include!() of shared files between packages. Serializing
    // makes it slower, but is the only safe way to prevent concurrent
    // modification.
    let _lock = LockServerClient::lock(&lock_addr.parse()?, "global")?;

    // Map of files that have been modified.
    let mut files = HashMap::new();

    if !args.can_run_rustfix(gctx)? {
        // This fix should not be run. Skipping...
        // We still need to run rustc at least once to make sure any potential
        // rmeta gets generated, and diagnostics get displayed.
        debug!("can't fix {filename:?}, running rustc: {rustc}");
        let last_output = rustc.output()?;
        let fixes = FixedCrate {
            files,
            first_output: last_output.clone(),
            last_output,
        };
        return Ok(fixes);
    }

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
    let max_iterations = gctx
        .get_env("CARGO_FIX_MAX_RETRIES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(4);
    let mut last_output;
    let mut last_made_changes;
    let mut first_output = None;
    let mut current_iteration = 0;
    loop {
        for file in files.values_mut() {
            // We'll generate new errors below.
            file.errors_applying_fixes.clear();
        }
        (last_output, last_made_changes) =
            rustfix_and_fix(&mut files, rustc, filename, args, gctx)?;
        if current_iteration == 0 {
            first_output = Some(last_output.clone());
        }
        let mut progress_yet_to_be_made = false;
        for (path, file) in files.iter_mut() {
            if file.errors_applying_fixes.is_empty() {
                continue;
            }
            debug!("had rustfix apply errors in {path:?} {file:?}");
            // If anything was successfully fixed *and* there's at least one
            // error, then assume the error was spurious and we'll try again on
            // the next iteration.
            if last_made_changes {
                progress_yet_to_be_made = true;
            }
        }
        if !progress_yet_to_be_made {
            break;
        }
        current_iteration += 1;
        if current_iteration >= max_iterations {
            break;
        }
    }
    if last_made_changes {
        debug!("calling rustc one last time for final results: {rustc}");
        last_output = rustc.output()?;
    }

    // Any errors still remaining at this point need to be reported as probably
    // bugs in Cargo and/or rustfix.
    for (path, file) in files.iter_mut() {
        for error in file.errors_applying_fixes.drain(..) {
            Message::ReplaceFailed {
                file: path.clone(),
                message: error,
            }
            .post(gctx)?;
        }
    }

    Ok(FixedCrate {
        files,
        first_output: first_output.expect("at least one iteration"),
        last_output,
    })
}

/// Executes `rustc` to apply one round of suggestions to the crate in question.
///
/// This will fill in the `fixes` map with original code, suggestions applied,
/// and any errors encountered while fixing files.
fn rustfix_and_fix(
    files: &mut HashMap<String, FixedFile>,
    rustc: &ProcessBuilder,
    filename: &Path,
    args: &FixArgs,
    gctx: &GlobalContext,
) -> CargoResult<(Output, bool)> {
    // If not empty, filter by these lints.
    // TODO: implement a way to specify this.
    let only = HashSet::new();

    debug!("calling rustc to collect suggestions and validate previous fixes: {rustc}");
    let output = rustc.output()?;

    // If rustc didn't succeed for whatever reasons then we're very likely to be
    // looking at otherwise broken code. Let's not make things accidentally
    // worse by applying fixes where a bug could cause *more* broken code.
    // Instead, punt upwards which will reexec rustc over the original code,
    // displaying pretty versions of the diagnostics we just read out.
    if !output.status.success() && gctx.get_env_os(BROKEN_CODE_ENV_INTERNAL).is_none() {
        debug!(
            "rustfixing `{:?}` failed, rustc exited with {:?}",
            filename,
            output.status.code()
        );
        return Ok((output, false));
    }

    let fix_mode = gctx
        .get_env_os("__CARGO_FIX_YOLO")
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
    // It's safe since we won't read any content under home dir.
    let home_path = gctx.home().as_path_unlocked();
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

        let file_path = Path::new(&file_name);
        // Do not write into registry cache. See rust-lang/cargo#9857.
        if file_path.starts_with(home_path) {
            continue;
        }
        // Do not write into standard library source. See rust-lang/cargo#9857.
        if let Some(sysroot) = args.sysroot.as_deref() {
            if file_path.starts_with(sysroot) {
                continue;
            }
        }

        if !file_names.clone().all(|f| f == &file_name) {
            trace!("rejecting as it changes multiple files: {:?}", suggestion);
            continue;
        }

        trace!("adding suggestion for {:?}: {:?}", file_name, suggestion);
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

    let mut made_changes = false;
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
        let fixed_file = files.entry(file.clone()).or_insert_with(|| FixedFile {
            errors_applying_fixes: Vec::new(),
            fixes_applied: 0,
            original_code: code.clone(),
        });
        let mut fixed = CodeFix::new(&code);

        for suggestion in suggestions.iter().rev() {
            // As mentioned above in `rustfix_crate`,
            // we don't immediately warn about suggestions that fail to apply here,
            // and instead we save them off for later processing.
            //
            // However, we don't bother reporting conflicts that exactly match prior replacements.
            // This is currently done to reduce noise for things like rust-lang/rust#51211,
            // although it may be removed if that's fixed deeper in the compiler.
            match fixed.apply(suggestion) {
                Ok(()) => fixed_file.fixes_applied += 1,
                Err(rustfix::Error::AlreadyReplaced {
                    is_identical: true, ..
                }) => continue,
                Err(e) => fixed_file.errors_applying_fixes.push(e.to_string()),
            }
        }
        if fixed.modified() {
            made_changes = true;
            let new_code = fixed.finish()?;
            paths::write(&file, new_code)?;
        }
    }

    Ok((output, made_changes))
}

fn exit_with(status: ExitStatus) -> ! {
    #[cfg(unix)]
    {
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

fn log_failed_fix(
    gctx: &GlobalContext,
    krate: Option<String>,
    stderr: &[u8],
    status: ExitStatus,
) -> CargoResult<()> {
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
    // Include any abnormal messages (like an ICE or whatever).
    errors.extend(
        stderr
            .lines()
            .filter(|x| !x.starts_with('{'))
            .map(|x| x.to_string()),
    );

    let files = files.into_iter().collect();
    let abnormal_exit = if status.code().map_or(false, is_simple_exit_code) {
        None
    } else {
        Some(exit_status_to_string(status))
    };
    Message::FixFailed {
        files,
        krate,
        errors,
        abnormal_exit,
    }
    .post(gctx)?;

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
    /// Path to host sysroot.
    sysroot: Option<PathBuf>,
}

impl FixArgs {
    fn get() -> CargoResult<FixArgs> {
        Self::from_args(env::args_os())
    }

    // This is a separate function so that we can use it in tests.
    fn from_args(argv: impl IntoIterator<Item = OsString>) -> CargoResult<Self> {
        let mut argv = argv.into_iter();
        let mut rustc = argv
            .nth(1)
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("expected rustc or `@path` as first argument"))?;
        let mut file = None;
        let mut enabled_edition = None;
        let mut other = Vec::new();

        let mut handle_arg = |arg: OsString| -> CargoResult<()> {
            let path = PathBuf::from(arg);
            if path.extension().and_then(|s| s.to_str()) == Some("rs") && path.exists() {
                file = Some(path);
                return Ok(());
            }
            if let Some(s) = path.to_str() {
                if let Some(edition) = s.strip_prefix("--edition=") {
                    enabled_edition = Some(edition.parse()?);
                    return Ok(());
                }
            }
            other.push(path.into());
            Ok(())
        };

        if let Some(argfile_path) = rustc.to_str().unwrap_or_default().strip_prefix("@") {
            // Because cargo in fix-proxy-mode might hit the command line size limit,
            // cargo fix need handle `@path` argfile for this special case.
            if argv.next().is_some() {
                bail!("argfile `@path` cannot be combined with other arguments");
            }
            let contents = fs::read_to_string(argfile_path)
                .with_context(|| format!("failed to read argfile at `{argfile_path}`"))?;
            let mut iter = contents.lines().map(OsString::from);
            rustc = iter
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("expected rustc as first argument"))?;
            for arg in iter {
                handle_arg(arg)?;
            }
        } else {
            for arg in argv {
                handle_arg(arg)?;
            }
        }

        let file = file.ok_or_else(|| anyhow::anyhow!("could not find .rs file in rustc args"))?;
        // ALLOWED: For the internal mechanism of `cargo fix` only.
        // Shouldn't be set directly by anyone.
        #[allow(clippy::disallowed_methods)]
        let idioms = env::var(IDIOMS_ENV_INTERNAL).is_ok();

        // ALLOWED: For the internal mechanism of `cargo fix` only.
        // Shouldn't be set directly by anyone.
        #[allow(clippy::disallowed_methods)]
        let prepare_for_edition = env::var(EDITION_ENV_INTERNAL).ok().map(|v| {
            let enabled_edition = enabled_edition.unwrap_or(Edition::Edition2015);
            let mode = EditionFixMode::from_str(&v);
            mode.next_edition(enabled_edition)
        });

        // ALLOWED: For the internal mechanism of `cargo fix` only.
        // Shouldn't be set directly by anyone.
        #[allow(clippy::disallowed_methods)]
        let sysroot = env::var_os(SYSROOT_INTERNAL).map(PathBuf::from);

        Ok(FixArgs {
            file,
            prepare_for_edition,
            idioms,
            enabled_edition,
            other,
            rustc,
            sysroot,
        })
    }

    fn apply(&self, cmd: &mut ProcessBuilder) {
        cmd.arg(&self.file);
        cmd.args(&self.other);
        if self.prepare_for_edition.is_some() {
            // When migrating an edition, we don't want to fix other lints as
            // they can sometimes add suggestions that fail to apply, causing
            // the entire migration to fail. But those lints aren't needed to
            // migrate.
            cmd.arg("--cap-lints=allow");
        } else {
            // This allows `cargo fix` to work even if the crate has #[deny(warnings)].
            cmd.arg("--cap-lints=warn");
        }
        if let Some(edition) = self.enabled_edition {
            cmd.arg("--edition").arg(edition.to_string());
            if self.idioms && edition.supports_idiom_lint() {
                cmd.arg(format!("-Wrust-{}-idioms", edition));
            }
        }

        if let Some(edition) = self.prepare_for_edition {
            edition.force_warn_arg(cmd);
        }
    }

    /// Validates the edition, and sends a message indicating what is being
    /// done. Returns a flag indicating whether this fix should be run.
    fn can_run_rustfix(&self, gctx: &GlobalContext) -> CargoResult<bool> {
        let Some(to_edition) = self.prepare_for_edition else {
            return Message::Fixing {
                file: self.file.display().to_string(),
            }
            .post(gctx)
            .and(Ok(true));
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
        if !to_edition.is_stable() && !gctx.nightly_features_allowed {
            let message = format!(
                "`{file}` is on the latest edition, but trying to \
                 migrate to edition {to_edition}.\n\
                 Edition {to_edition} is unstable and not allowed in \
                 this release, consider trying the nightly release channel.",
                file = self.file.display(),
                to_edition = to_edition
            );
            return Message::EditionAlreadyEnabled {
                message,
                edition: to_edition.previous().unwrap(),
            }
            .post(gctx)
            .and(Ok(false)); // Do not run rustfix for this the edition.
        }
        let from_edition = self.enabled_edition.unwrap_or(Edition::Edition2015);
        if from_edition == to_edition {
            let message = format!(
                "`{}` is already on the latest edition ({}), \
                 unable to migrate further",
                self.file.display(),
                to_edition
            );
            Message::EditionAlreadyEnabled {
                message,
                edition: to_edition,
            }
            .post(gctx)
        } else {
            Message::Migrating {
                file: self.file.display().to_string(),
                from_edition,
                to_edition,
            }
            .post(gctx)
        }
        .and(Ok(true))
    }
}

#[cfg(test)]
mod tests {
    use super::FixArgs;
    use std::ffi::OsString;
    use std::io::Write as _;
    use std::path::PathBuf;

    #[test]
    fn get_fix_args_from_argfile() {
        let mut temp = tempfile::Builder::new().tempfile().unwrap();
        let main_rs = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();

        let content = format!("/path/to/rustc\n{}\nfoobar\n", main_rs.path().display());
        temp.write_all(content.as_bytes()).unwrap();

        let argfile = format!("@{}", temp.path().display());
        let args = ["cargo", &argfile];
        let fix_args = FixArgs::from_args(args.map(|x| x.into())).unwrap();
        assert_eq!(fix_args.rustc, PathBuf::from("/path/to/rustc"));
        assert_eq!(fix_args.file, main_rs.path());
        assert_eq!(fix_args.other, vec![OsString::from("foobar")]);
    }

    #[test]
    fn get_fix_args_from_argfile_with_extra_arg() {
        let mut temp = tempfile::Builder::new().tempfile().unwrap();
        let main_rs = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();

        let content = format!("/path/to/rustc\n{}\nfoobar\n", main_rs.path().display());
        temp.write_all(content.as_bytes()).unwrap();

        let argfile = format!("@{}", temp.path().display());
        let args = ["cargo", &argfile, "boo!"];
        match FixArgs::from_args(args.map(|x| x.into())) {
            Err(e) => assert_eq!(
                e.to_string(),
                "argfile `@path` cannot be combined with other arguments"
            ),
            Ok(_) => panic!("should fail"),
        }
    }
}
