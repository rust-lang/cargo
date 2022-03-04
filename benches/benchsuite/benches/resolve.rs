use cargo::core::compiler::{CompileKind, RustcTargetData};
use cargo::core::resolver::features::{CliFeatures, FeatureOpts, FeatureResolver, ForceAllTargets};
use cargo::core::resolver::{HasDevUnits, ResolveBehavior};
use cargo::core::{PackageIdSpec, Workspace};
use cargo::ops::WorkspaceResolve;
use cargo::Config;
use criterion::{criterion_group, criterion_main, Criterion};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

// This is an arbitrary commit that existed when I started. This helps
// ensure consistent results. It can be updated if needed, but that can
// make it harder to compare results with older versions of cargo.
const CRATES_IO_COMMIT: &str = "85f7bfd61ea4fee08ec68c468762e886b2aebec6";

fn setup() {
    create_home();
    create_target_dir();
    clone_index();
    unpack_workspaces();
}

fn root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    p.push("bench");
    p
}

fn target_dir() -> PathBuf {
    let mut p = root();
    p.push("target");
    p
}

fn cargo_home() -> PathBuf {
    let mut p = root();
    p.push("chome");
    p
}

fn index() -> PathBuf {
    let mut p = root();
    p.push("index");
    p
}

fn workspaces_path() -> PathBuf {
    let mut p = root();
    p.push("workspaces");
    p
}

fn registry_url() -> Url {
    Url::from_file_path(index()).unwrap()
}

fn create_home() {
    let home = cargo_home();
    if !home.exists() {
        fs::create_dir_all(&home).unwrap();
    }
    fs::write(
        home.join("config.toml"),
        format!(
            r#"
                [source.crates-io]
                replace-with = 'local-snapshot'

                [source.local-snapshot]
                registry = '{}'
            "#,
            registry_url()
        ),
    )
    .unwrap();
}

fn create_target_dir() {
    // This is necessary to ensure the .rustc_info.json file is written.
    // Otherwise it won't be written, and it is very expensive to create.
    if !target_dir().exists() {
        std::fs::create_dir_all(target_dir()).unwrap();
    }
}

/// This clones crates.io at a specific point in time into tmp/index.
fn clone_index() {
    let index = index();
    let maybe_git = |command: &str| {
        let status = Command::new("git")
            .current_dir(&index)
            .args(command.split_whitespace().collect::<Vec<_>>())
            .status()
            .expect("git should be installed");
        status.success()
    };
    let git = |command: &str| {
        if !maybe_git(command) {
            panic!("failed to run git command: {}", command);
        }
    };
    if index.exists() {
        if maybe_git(&format!(
            "rev-parse -q --verify {}^{{commit}}",
            CRATES_IO_COMMIT
        )) {
            // Already fetched.
            return;
        }
    } else {
        fs::create_dir_all(&index).unwrap();
        git("init --bare");
        git("remote add origin https://github.com/rust-lang/crates.io-index");
    }
    git(&format!("fetch origin {}", CRATES_IO_COMMIT));
    git("branch -f master FETCH_HEAD");
}

/// This unpacks the compressed workspace skeletons into tmp/workspaces.
fn unpack_workspaces() {
    let ws_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("workspaces");
    let archives = fs::read_dir(ws_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension() == Some(std::ffi::OsStr::new("tgz")));
    for archive in archives {
        let name = archive.file_stem().unwrap();
        let f = fs::File::open(&archive).unwrap();
        let f = flate2::read::GzDecoder::new(f);
        let dest = workspaces_path().join(&name);
        if dest.exists() {
            fs::remove_dir_all(&dest).unwrap();
        }
        let mut archive = tar::Archive::new(f);
        archive.unpack(workspaces_path()).unwrap();
    }
}

struct ResolveInfo<'cfg> {
    ws: Workspace<'cfg>,
    requested_kinds: [CompileKind; 1],
    target_data: RustcTargetData<'cfg>,
    cli_features: CliFeatures,
    specs: Vec<PackageIdSpec>,
    has_dev_units: HasDevUnits,
    force_all_targets: ForceAllTargets,
    ws_resolve: WorkspaceResolve<'cfg>,
}

/// Vec of `(ws_name, ws_root)`.
fn workspaces() -> Vec<(String, PathBuf)> {
    // CARGO_BENCH_WORKSPACES can be used to override, otherwise it just uses
    // the workspaces in the workspaces directory.
    let mut ps: Vec<_> = match std::env::var_os("CARGO_BENCH_WORKSPACES") {
        Some(s) => std::env::split_paths(&s).collect(),
        None => fs::read_dir(workspaces_path())
            .unwrap()
            .map(|e| e.unwrap().path())
            // These currently fail in most cases on Windows due to long
            // filenames in the git checkouts.
            .filter(|p| {
                !(cfg!(windows)
                    && matches!(p.file_name().unwrap().to_str().unwrap(), "servo" | "tikv"))
            })
            .collect(),
    };
    // Sort so it is consistent.
    ps.sort();
    ps.into_iter()
        .map(|p| (p.file_name().unwrap().to_str().unwrap().to_owned(), p))
        .collect()
}

/// Helper for resolving a workspace. This will run the resolver once to
/// download everything, and returns all the data structures that are used
/// during resolution.
fn do_resolve<'cfg>(config: &'cfg Config, ws_root: &Path) -> ResolveInfo<'cfg> {
    let requested_kinds = [CompileKind::Host];
    let ws = cargo::core::Workspace::new(&ws_root.join("Cargo.toml"), config).unwrap();
    let target_data = RustcTargetData::new(&ws, &requested_kinds).unwrap();
    let cli_features = CliFeatures::from_command_line(&[], false, true).unwrap();
    let pkgs = cargo::ops::Packages::Default;
    let specs = pkgs.to_package_id_specs(&ws).unwrap();
    let has_dev_units = HasDevUnits::Yes;
    let force_all_targets = ForceAllTargets::No;
    // Do an initial run to download anything necessary so that it does
    // not confuse criterion's warmup.
    let ws_resolve = cargo::ops::resolve_ws_with_opts(
        &ws,
        &target_data,
        &requested_kinds,
        &cli_features,
        &specs,
        has_dev_units,
        force_all_targets,
    )
    .unwrap();
    ResolveInfo {
        ws,
        requested_kinds,
        target_data,
        cli_features,
        specs,
        has_dev_units,
        force_all_targets,
        ws_resolve,
    }
}

/// Creates a new Config.
///
/// This is separate from `do_resolve` to deal with the ownership and lifetime.
fn make_config(ws_root: &Path) -> Config {
    let shell = cargo::core::Shell::new();
    let mut config = cargo::util::Config::new(shell, ws_root.to_path_buf(), cargo_home());
    // Configure is needed to set the target_dir which is needed to write
    // the .rustc_info.json file which is very expensive.
    config
        .configure(
            0,
            false,
            None,
            false,
            false,
            false,
            &Some(target_dir()),
            &[],
            &[],
        )
        .unwrap();
    config
}

/// Benchmark of the full `resolve_ws_with_opts` which runs the resolver
/// twice, the feature resolver, and more. This is a major component of a
/// regular cargo build.
fn resolve_ws(c: &mut Criterion) {
    setup();
    let mut group = c.benchmark_group("resolve_ws");
    for (ws_name, ws_root) in workspaces() {
        let config = make_config(&ws_root);
        // The resolver info is initialized only once in a lazy fashion. This
        // allows criterion to skip this workspace if the user passes a filter
        // on the command-line (like `cargo bench -- resolve_ws/tikv`).
        //
        // Due to the way criterion works, it tends to only run the inner
        // iterator once, and we don't want to call `do_resolve` in every
        // "step", since that would just be some useless work.
        let mut lazy_info = None;
        group.bench_function(&ws_name, |b| {
            let ResolveInfo {
                ws,
                requested_kinds,
                target_data,
                cli_features,
                specs,
                has_dev_units,
                force_all_targets,
                ..
            } = lazy_info.get_or_insert_with(|| do_resolve(&config, &ws_root));
            b.iter(|| {
                cargo::ops::resolve_ws_with_opts(
                    ws,
                    target_data,
                    requested_kinds,
                    cli_features,
                    specs,
                    *has_dev_units,
                    *force_all_targets,
                )
                .unwrap();
            })
        });
    }
    group.finish();
}

/// Benchmark of the feature resolver.
fn feature_resolver(c: &mut Criterion) {
    setup();
    let mut group = c.benchmark_group("feature_resolver");
    for (ws_name, ws_root) in workspaces() {
        let config = make_config(&ws_root);
        let mut lazy_info = None;
        group.bench_function(&ws_name, |b| {
            let ResolveInfo {
                ws,
                requested_kinds,
                target_data,
                cli_features,
                specs,
                has_dev_units,
                ws_resolve,
                ..
            } = lazy_info.get_or_insert_with(|| do_resolve(&config, &ws_root));
            b.iter(|| {
                let feature_opts = FeatureOpts::new_behavior(ResolveBehavior::V2, *has_dev_units);
                FeatureResolver::resolve(
                    ws,
                    target_data,
                    &ws_resolve.targeted_resolve,
                    &ws_resolve.pkg_set,
                    cli_features,
                    specs,
                    requested_kinds,
                    feature_opts,
                )
                .unwrap();
            })
        });
    }
    group.finish();
}

// Criterion complains about the measurement time being too small, but the
// measurement time doesn't seem important to me, what is more important is
// the number of iterations which defaults to 100, which seems like a
// reasonable default. Otherwise, the measurement time would need to be
// changed per workspace. We wouldn't want to spend 60s on every workspace,
// that would take too long and isn't necessary for the smaller workspaces.
criterion_group!(benches, resolve_ws, feature_resolver);
criterion_main!(benches);
