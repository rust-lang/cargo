use benchsuite::fixtures;
use cargo::core::compiler::{CompileKind, RustcTargetData};
use cargo::core::resolver::features::{FeatureOpts, FeatureResolver};
use cargo::core::resolver::{CliFeatures, ForceAllTargets, HasDevUnits, ResolveBehavior};
use cargo::core::{PackageIdSpec, Workspace};
use cargo::ops::WorkspaceResolve;
use cargo::Config;
use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;

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

/// Helper for resolving a workspace. This will run the resolver once to
/// download everything, and returns all the data structures that are used
/// during resolution.
fn do_resolve<'cfg>(config: &'cfg Config, ws_root: &Path) -> ResolveInfo<'cfg> {
    let requested_kinds = [CompileKind::Host];
    let ws = Workspace::new(&ws_root.join("Cargo.toml"), config).unwrap();
    let mut target_data = RustcTargetData::new(&ws, &requested_kinds).unwrap();
    let cli_features = CliFeatures::from_command_line(&[], false, true).unwrap();
    let pkgs = cargo::ops::Packages::Default;
    let specs = pkgs.to_package_id_specs(&ws).unwrap();
    let has_dev_units = HasDevUnits::Yes;
    let force_all_targets = ForceAllTargets::No;
    let max_rust_version = None;
    // Do an initial run to download anything necessary so that it does
    // not confuse criterion's warmup.
    let ws_resolve = cargo::ops::resolve_ws_with_opts(
        &ws,
        &mut target_data,
        &requested_kinds,
        &cli_features,
        &specs,
        has_dev_units,
        force_all_targets,
        max_rust_version,
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
fn resolve_ws(c: &mut Criterion) {
    let fixtures = fixtures!();
    let mut group = c.benchmark_group("resolve_ws");
    for (ws_name, ws_root) in fixtures.workspaces() {
        let config = fixtures.make_config(&ws_root);
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
            let max_rust_version = None;
            b.iter(|| {
                cargo::ops::resolve_ws_with_opts(
                    ws,
                    target_data,
                    requested_kinds,
                    cli_features,
                    specs,
                    *has_dev_units,
                    *force_all_targets,
                    max_rust_version,
                )
                .unwrap();
            })
        });
    }
    group.finish();
}

/// Benchmark of the feature resolver.
fn feature_resolver(c: &mut Criterion) {
    let fixtures = fixtures!();
    let mut group = c.benchmark_group("feature_resolver");
    for (ws_name, ws_root) in fixtures.workspaces() {
        let config = fixtures.make_config(&ws_root);
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
