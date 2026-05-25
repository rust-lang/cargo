use std::path::Path;

use cargo_util_schemas::manifest;
use cargo_util_schemas::manifest::TomlPackageBuild;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Patch;
use cargo_util_terminal::report::Snippet;
use indexmap::IndexMap;
use tracing::{debug, instrument, trace};

use super::STYLE;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::core::PackageId;
use crate::core::Workspace;
use crate::core::compiler::BuildContext;
use crate::core::compiler::BuildRunner;
use crate::core::compiler::Unit;
use crate::core::compiler::unused_deps::DependenciesState;
use crate::core::compiler::unused_deps::UnusedDepState;
use crate::core::dependency::DepKind;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::Lint;
use crate::diagnostics::LintLevel;
use crate::diagnostics::LintLevelProduct;
use crate::diagnostics::get_key_value_span;
use crate::diagnostics::rel_cwd_manifest_path;

pub static LINT: &Lint = &Lint {
    name: "unused_dependencies",
    desc: "unused dependency",
    primary_group: &STYLE,
    msrv: Some(super::CARGO_LINTS_MSRV),
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for dependencies that are not used by any of the cargo targets.

### Why it is bad

Slows down compilation time.

### Drawbacks

The lint is only emitted in specific circumstances as multiple cargo targets exist for the
different dependencies tables and they must all be built to know if a dependency is unused.
Currently, only the selected packages are checked and not all `path` dependencies like most lints.
The cargo target selection flags,
independent of which packages are selected, determine which dependencies tables are checked.
As there is no way to select all cargo targets that use `[dev-dependencies]`,
they are unchecked.

Examples:
- `cargo check` will lint `[build-dependencies]` and `[dependencies]`
- `cargo check --all-targets` will still only lint `[build-dependencies]` and `[dependencies]` and not `[dev-dependencoes]`
- `cargo check --bin foo` will not lint `[dependencies]` even if `foo` is the only bin though `[build-dependencies]` will be checked
- `cargo check -p foo` will not lint any dependencies tables for the `path` dependency `bar` even if `bar` only has a `[lib]`

There can be false positives when depending on a transitive dependency to activate a feature.

For false positives from pinning the version of a transitive dependency in `Cargo.toml`,
move the dependency to the `target."cfg(false)".dependencies` table.

### Example

```toml
[package]
name = "foo"

[dependencies]
unused = "1"
```

Should be written as:

```toml
[package]
name = "foo"
```
"#,
    ),
};

/// Lint for `[build-dependencies]` without a `build.rs`
///
/// These are always unused.
///
/// This must be determined independent of the compiler since there are no build targets to pass to
/// rustc to report on these.
#[instrument(skip_all)]
pub(crate) fn lint_package(
    _ws: &Workspace<'_>,
    pkg: &Package,
    manifest_path: &Path,
    level: LintLevelProduct,
    pkg_stats: &mut DiagnosticStats,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let LintLevelProduct {
        level: lint_level,
        source,
    } = level;

    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);

    let manifest = pkg.manifest();
    let Some(package) = &manifest.normalized_toml().package else {
        return Ok(());
    };
    if package.build != Some(TomlPackageBuild::Auto(false)) {
        return Ok(());
    }

    let document = manifest.document();
    let contents = manifest.contents();

    for (i, dep_name) in manifest
        .normalized_toml()
        .build_dependencies()
        .iter()
        .flat_map(|m| m.keys())
        .enumerate()
    {
        let level = lint_level.to_diagnostic_level();
        let emitted_source = LINT.emitted_source(lint_level, source);

        let mut primary = Group::with_title(level.primary_title(LINT.desc));
        if let Some(document) = document
            && let Some(contents) = contents
            && let Some(span) = get_key_value_span(document, &["build-dependencies", dep_name])
        {
            let span = span.key.start..span.value.end;
            primary = primary.element(
                Snippet::source(contents)
                    .path(&manifest_path)
                    .annotation(AnnotationKind::Primary.span(span)),
            );
        } else {
            primary = primary.element(Origin::path(&manifest_path));
        }
        if i == 0 {
            primary = primary.element(Level::NOTE.message(emitted_source));
        }
        let mut report = vec![primary];
        if let Some(document) = document
            && let Some(contents) = contents
            && let Some(span) = get_key_value_span(document, &["build-dependencies", dep_name])
        {
            let span = span.key.start..span.value.end;
            let mut help = Group::with_title(Level::HELP.secondary_title("remove the dependency"));
            help = help.element(
                Snippet::source(contents)
                    .path(&manifest_path)
                    .patch(Patch::new(span, "")),
            );
            report.push(help);
        }

        pkg_stats.record_lint(lint_level);
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}

#[instrument(skip_all)]
pub fn lint_build_results(
    build_runner: &BuildRunner<'_, '_>,
    global_stats: &mut DiagnosticStats,
) -> CargoResult<()> {
    for (pkg_id, states) in &build_runner.unused_dep_state.states {
        let Some(pkg) = get_package(&build_runner.unused_dep_state, pkg_id) else {
            continue;
        };
        let toml_lints = pkg
            .manifest()
            .normalized_toml()
            .lints
            .clone()
            .map(|lints| lints.lints)
            .unwrap_or(manifest::TomlLints::default());
        let cargo_lints = toml_lints
            .get("cargo")
            .cloned()
            .unwrap_or(manifest::TomlToolLints::default());
        let level = LINT.level(
            &cargo_lints,
            pkg.rust_version(),
            pkg.manifest().unstable_features(),
        );
        if level.level == LintLevel::Allow {
            for (dep_kind, state) in states.iter() {
                for ext in state.unused_externs.iter().flatten() {
                    debug!(
                        "pkg {} v{} ({dep_kind:?}): ignoring unused extern `{ext}`, lint is allowed",
                        pkg_id.name(),
                        pkg_id.version(),
                    );
                }
            }
            continue;
        }

        lint_package_build_results(build_runner, pkg, states, level, global_stats)?;
    }
    Ok(())
}

fn lint_package_build_results(
    build_runner: &BuildRunner<'_, '_>,
    pkg: &Package,
    states: &IndexMap<DepKind, DependenciesState>,
    level: LintLevelProduct,
    global_stats: &mut DiagnosticStats,
) -> CargoResult<()> {
    let mut lint_count = 0;
    let LintLevelProduct {
        level: lint_level,
        source,
    } = level;
    let manifest_path = rel_cwd_manifest_path(pkg.manifest_path(), build_runner.bcx.gctx);
    let pkg_id = pkg.package_id();
    for (dep_kind, state) in states.iter() {
        for ext in state.unused_externs.iter().flatten() {
            let mut used_in_dev = false;
            match dep_kind {
                DepKind::Normal => {
                    if let Some(state) = states.get(&DepKind::Development)
                        && state
                            .unused_externs
                            .as_ref()
                            .is_some_and(|ue| !ue.contains(ext))
                    {
                        used_in_dev = true;
                    }
                }
                DepKind::Development => {
                    if let Some(state) = states.get(&DepKind::Normal)
                        && state.externs.contains_key(ext)
                    {
                        trace!(
                            "pkg {} v{} ({dep_kind:?}): ignoring unused extern `{ext}`, inherited from normal dependency",
                            pkg_id.name(),
                            pkg_id.version(),
                        );
                        continue;
                    }
                }
                DepKind::Build => {}
            }
            let Some(extern_state) = state.externs.get(ext) else {
                // not one we care to report
                debug!(
                    "pkg {} v{} ({dep_kind:?}): ignoring unused extern `{ext}`, untracked dependent",
                    pkg_id.name(),
                    pkg_id.version(),
                );
                continue;
            };
            if state.seen_units.len() != state.needed_units {
                debug_assert_ne!(state.externs.len(), 0, "assumes tracked is checked first");
                // Some compilations errored without printing the unused externs.
                // Don't print the warning in order to reduce false positive
                // spam during errors.
                debug!(
                    "pkg {} v{} ({dep_kind:?}): ignoring unused extern `{ext}`, {} outstanding units",
                    pkg_id.name(),
                    pkg_id.version(),
                    state.needed_units - state.seen_units.len()
                );
                continue;
            }
            if is_transitive_dep(&extern_state.unit, &state.seen_units, build_runner.bcx) {
                debug!(
                    "pkg {} v{} ({dep_kind:?}): ignoring unused extern `{ext}`, may be activating features",
                    pkg_id.name(),
                    pkg_id.version(),
                );
                continue;
            }

            // Implicitly added dependencies (in the same crate) aren't interesting
            let dependency = if let Some(dependency) = &extern_state.manifest_deps {
                dependency
            } else {
                continue;
            };
            for dependency in dependency {
                let manifest = pkg.manifest();
                let document = manifest.document();
                let contents = manifest.contents();
                let level = lint_level.to_diagnostic_level();
                let emitted_source = LINT.emitted_source(lint_level, source);
                let toml_path = dependency.toml_path();

                let mut primary = Group::with_title(level.primary_title(LINT.desc));
                if let Some(document) = document
                    && let Some(contents) = contents
                    && let Some(span) = get_key_value_span(document, &toml_path)
                {
                    let span = span.key.start..span.value.end;
                    primary = primary.element(
                        Snippet::source(contents)
                            .path(&manifest_path)
                            .annotation(AnnotationKind::Primary.span(span)),
                    );
                } else {
                    primary = primary.element(Origin::path(&manifest_path));
                }
                if lint_count == 0 {
                    primary = primary.element(Level::NOTE.message(emitted_source));
                }
                lint_count += 1;
                let mut report = vec![primary];
                if let Some(document) = document
                    && let Some(contents) = contents
                    && let Some(span) = get_key_value_span(document, &toml_path)
                {
                    let span = span.key.start..span.value.end;
                    let mut help =
                        Group::with_title(Level::HELP.secondary_title("remove the dependency"));
                    help = help.element(
                        Snippet::source(contents)
                            .path(&manifest_path)
                            .patch(Patch::new(span, "")),
                    );
                    report.push(help);
                }
                if used_in_dev {
                    let help = Group::with_title(Level::HELP.secondary_title(
                        "to still use for development builds, move to `dev-dependencies`",
                    ));
                    report.push(help);
                }

                global_stats.record_lint(lint_level);
                build_runner
                    .bcx
                    .gctx
                    .shell()
                    .print_report(&report, lint_level.force())?;
            }
        }
    }
    Ok(())
}

fn get_package<'s>(
    unused_dep_state: &'s UnusedDepState,
    pkg_id: &PackageId,
) -> Option<&'s Package> {
    let state = unused_dep_state.states.get(pkg_id)?;
    let mut iter = state.values();
    let state = iter.next()?;
    let mut iter = state.seen_units.iter();
    let unit = iter.next()?;
    Some(&unit.pkg)
}

#[instrument(skip_all)]
fn is_transitive_dep(
    direct_dep_unit: &Unit,
    seen_units: &Vec<Unit>,
    bcx: &BuildContext<'_, '_>,
) -> bool {
    let mut queue = std::collections::VecDeque::new();
    for root_unit in seen_units {
        for unit_dep in &bcx.unit_graph[root_unit] {
            if root_unit.pkg.package_id() == unit_dep.unit.pkg.package_id() {
                continue;
            }
            if unit_dep.unit == *direct_dep_unit {
                continue;
            }
            queue.push_back(&unit_dep.unit);
        }
    }

    while let Some(dep_unit) = queue.pop_front() {
        for unit_dep in &bcx.unit_graph[dep_unit] {
            if unit_dep.unit == *direct_dep_unit {
                return true;
            }
            queue.push_back(&unit_dep.unit);
        }
    }

    false
}
