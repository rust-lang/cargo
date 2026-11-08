use benchsuite::{Fixtures, WorkspacePath, fixtures};
use cargo::GlobalContext;
use cargo::compiler::{CompileKind, RustcTargetData};
use cargo::ops::WorkspaceResolve;
use cargo::resolver::features::{FeatureOpts, FeatureResolver};
use cargo::resolver::{CliFeatures, ForceAllTargets, HasDevUnits, ResolveBehavior};
use cargo::workspace::{PackageIdSpec, Workspace};
use std::path::Path;
use std::sync::LazyLock;

static FIXTURES: LazyLock<Fixtures> = LazyLock::new(|| fixtures!());

struct ResolveInfo<'gctx> {
    ws: Workspace<'gctx>,
    requested_kinds: [CompileKind; 1],
    target_data: RustcTargetData<'gctx>,
    cli_features: CliFeatures,
    specs: Vec<PackageIdSpec>,
    has_dev_units: HasDevUnits,
    force_all_targets: ForceAllTargets,
    ws_resolve: WorkspaceResolve<'gctx>,
}

/// Helper for resolving a workspace. This will run the resolver once to
/// download everything, and returns all the data structures that are used
/// during resolution.
fn do_resolve<'gctx>(gctx: &'gctx GlobalContext, ws_root: &Path) -> ResolveInfo<'gctx> {
    let requested_kinds = [CompileKind::Host];
    let ws = Workspace::new(&ws_root.join("Cargo.toml"), gctx).unwrap();
    let mut target_data = RustcTargetData::new(&ws, &requested_kinds).unwrap();
    let cli_features = CliFeatures::from_command_line(&[], false, true).unwrap();
    let pkgs = cargo::ops::Packages::Default;
    let specs = pkgs.to_package_id_specs(&ws).unwrap();
    let has_dev_units = HasDevUnits::Yes;
    let force_all_targets = ForceAllTargets::No;
    // Do an initial run to download anything necessary so that it does
    // not affect the benchmark.
    let dry_run = false;
    let ws_resolve = cargo::ops::resolve_ws_with_opts(
        &ws,
        &mut target_data,
        &requested_kinds,
        &cli_features,
        &specs,
        has_dev_units,
        force_all_targets,
        dry_run,
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

/// Benchmark of the full `resolve_ws_with_opts` which runs the resolver
/// twice, the feature resolver, and more. This is a major component of a
/// regular cargo build.
#[divan::bench(args = FIXTURES.workspaces())]
fn resolve_ws(b: divan::Bencher<'_, '_>, WorkspacePath(ws_root): &WorkspacePath) {
    let gctx = FIXTURES.make_context(ws_root);
    let dry_run = false;
    let ResolveInfo {
        ws,
        requested_kinds,
        target_data,
        cli_features,
        specs,
        has_dev_units,
        force_all_targets,
        ..
    } = &mut do_resolve(&gctx, ws_root);
    b.bench_local(|| {
        cargo::ops::resolve_ws_with_opts(
            ws,
            target_data,
            requested_kinds,
            cli_features,
            specs,
            *has_dev_units,
            *force_all_targets,
            dry_run,
        )
        .unwrap();
    });
}

/// Benchmark of the feature resolver.
#[divan::bench(args = FIXTURES.workspaces())]
fn feature_resolver(b: divan::Bencher<'_, '_>, WorkspacePath(ws_root): &WorkspacePath) {
    let gctx = FIXTURES.make_context(ws_root);
    let ResolveInfo {
        ws,
        requested_kinds,
        target_data,
        cli_features,
        specs,
        has_dev_units,
        ws_resolve,
        ..
    } = &mut do_resolve(&gctx, ws_root);
    b.bench_local(|| {
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
    });
}

fn main() {
    divan::main();
}
