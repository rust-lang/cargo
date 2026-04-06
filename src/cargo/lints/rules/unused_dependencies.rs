use std::path::Path;

use cargo_util_schemas::manifest::TomlPackageBuild;
use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Patch;
use cargo_util_terminal::report::Snippet;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::STYLE;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

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

### Configuration

- `ignore`: a list of dependency names to allow the lint on
"#,
    ),
};

/// Lint for `[build-dependencies]` without a `build.rs`
///
/// These are always unused.
///
/// This must be determined independent of the compiler since there are no build targets to pass to
/// rustc to report on these.
pub fn unused_build_dependencies_no_build_rs(
    pkg: &Package,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        pkg.rust_version(),
        pkg.manifest().unstable_features(),
    );

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

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

    let mut ignore = Vec::new();
    if let Some(unused_dependencies) = cargo_lints.get("unused_dependencies") {
        if let Some(config) = unused_dependencies.config() {
            if let Some(config_ignore) = config.get("ignore") {
                if let Ok(config_ignore) =
                    toml::Value::try_into::<Vec<String>>(config_ignore.clone())
                {
                    ignore = config_ignore
                } else {
                    anyhow::bail!(
                        "`lints.cargo.unused_dependencies.ignore` must be a list of string"
                    );
                }
            }
        }
    }

    for (i, dep_name) in manifest
        .normalized_toml()
        .build_dependencies()
        .iter()
        .flat_map(|m| m.keys())
        .enumerate()
    {
        if ignore.contains(&dep_name) {
            continue;
        }

        let level = lint_level.to_diagnostic_level();
        let emitted_source = LINT.emitted_source(lint_level, reason);

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

        if lint_level.is_error() {
            *error_count += 1;
        }
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}
