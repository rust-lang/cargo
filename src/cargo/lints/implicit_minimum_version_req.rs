use std::collections::HashMap;
use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_platform::Platform;
use cargo_util_schemas::manifest::TomlDependency;
use cargo_util_schemas::manifest::TomlToolLints;
use toml::de::DeValue;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Manifest;
use crate::core::MaybePackage;
use crate::core::Package;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelReason;
use crate::lints::ManifestFor;
use crate::lints::get_key_value;
use crate::lints::rel_cwd_manifest_path;
use crate::util::OptVersionReq;

pub const LINT: Lint = Lint {
    name: "implicit_minimum_version_req",
    desc: "dependency version requirement without an explicit minimum version",
    groups: &[],
    default_level: LintLevel::Allow,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for dependency version requirements
that do not explicitly specify a full `major.minor.patch` version requirement,
such as `serde = "1"` or `serde = "1.0"`.

This lint currently only applies to caret requirements
(the [default requirements](specifying-dependencies.md#default-requirements)).

### Why it is bad

Version requirements without an explicit full version
can be misleading about the actual minimum supported version.
For example,
`serde = "1"` has an implicit minimum bound of `1.0.0`.
If your code actually requires features from `1.0.219`,
the implicit minimum bound of `1.0.0` gives a false impression about compatibility.

Specifying the full version helps with:

- Accurate minimum version documentation
- Better compatibility with `-Z minimal-versions`
- Clearer dependency constraints for consumers

### Drawbacks

Even with a fully specified version,
the minimum bound might still be incorrect if untested.
This lint helps make the minimum version requirement explicit
but doesn't guarantee correctness.

### Example

```toml
[dependencies]
serde = "1"
```

Should be written as a full specific version:

```toml
[dependencies]
serde = "1.0.219"
```
"#,
    ),
};

pub fn implicit_minimum_version_req(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = manifest.lint_level(cargo_lints, LINT);

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);

    match manifest {
        ManifestFor::Package(pkg) => {
            lint_package(pkg, manifest_path, lint_level, reason, error_count, gctx)
        }
        ManifestFor::Workspace(maybe_pkg) => lint_workspace(
            maybe_pkg,
            manifest_path,
            lint_level,
            reason,
            error_count,
            gctx,
        ),
    }
}

pub fn lint_package(
    pkg: &Package,
    manifest_path: String,
    lint_level: LintLevel,
    reason: LintLevelReason,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest = pkg.manifest();

    let document = manifest.document();
    let contents = manifest.contents();
    let target_key_for_platform = target_key_for_platform(&manifest);

    for dep in manifest.dependencies().iter() {
        let version_req = dep.version_req();
        let Some(suggested_req) = get_suggested_version_req(&version_req) else {
            continue;
        };

        let name_in_toml = dep.name_in_toml().as_str();
        let key_path =
            if let Some(cfg) = dep.platform().and_then(|p| target_key_for_platform.get(p)) {
                &["target", &cfg, dep.kind().kind_table(), name_in_toml][..]
            } else {
                &[dep.kind().kind_table(), name_in_toml][..]
            };

        let Some(span) = span_of_version_req(document, key_path) else {
            continue;
        };

        let report = report(
            lint_level,
            reason,
            span,
            contents,
            &manifest_path,
            &suggested_req,
        );

        if lint_level.is_error() {
            *error_count += 1;
        }
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}

pub fn lint_workspace(
    maybe_pkg: &MaybePackage,
    manifest_path: String,
    lint_level: LintLevel,
    reason: LintLevelReason,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let document = maybe_pkg.document();
    let contents = maybe_pkg.contents();
    let toml = match maybe_pkg {
        MaybePackage::Package(p) => p.manifest().normalized_toml(),
        MaybePackage::Virtual(vm) => vm.normalized_toml(),
    };
    let dep_iter = toml
        .workspace
        .as_ref()
        .and_then(|ws| ws.dependencies.as_ref())
        .into_iter()
        .flat_map(|deps| deps.iter())
        .map(|(name, dep)| {
            let name = name.as_str();
            let ver = match dep {
                TomlDependency::Simple(ver) => ver,
                TomlDependency::Detailed(detailed) => {
                    let Some(ver) = detailed.version.as_ref() else {
                        return (name, OptVersionReq::Any);
                    };
                    ver
                }
            };
            let req = semver::VersionReq::parse(ver)
                .map(Into::into)
                .unwrap_or(OptVersionReq::Any);
            (name, req)
        });

    for (name_in_toml, version_req) in dep_iter {
        let Some(suggested_req) = get_suggested_version_req(&version_req) else {
            continue;
        };

        let key_path = ["workspace", "dependencies", name_in_toml];

        let Some(span) = span_of_version_req(document, &key_path) else {
            continue;
        };

        let report = report(
            lint_level,
            reason,
            span,
            contents,
            &manifest_path,
            &suggested_req,
        );

        if lint_level.is_error() {
            *error_count += 1;
        }
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}

pub fn span_of_version_req<'doc>(
    document: &'doc toml::Spanned<toml::de::DeTable<'static>>,
    path: &[&str],
) -> Option<std::ops::Range<usize>> {
    let (_key, value) = get_key_value(document, path)?;

    match value.as_ref() {
        DeValue::String(_) => Some(value.span()),
        DeValue::Table(map) if map.get("workspace").is_some() => {
            // We only lint non-workspace-inherited dependencies
            None
        }
        DeValue::Table(map) => {
            let Some(v) = map.get("version") else {
                panic!("version must be specified or workspace-inherited");
            };
            Some(v.span())
        }
        _ => unreachable!("dependency must be string or table"),
    }
}

fn report<'a>(
    lint_level: LintLevel,
    reason: LintLevelReason,
    span: std::ops::Range<usize>,
    contents: &'a str,
    manifest_path: &str,
    suggested_req: &str,
) -> [Group<'a>; 2] {
    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);
    let replacement = format!(r#""{suggested_req}""#);
    let label = "missing full version components";
    let secondary_title = "consider specifying full `major.minor.patch` version components";
    [
        level.clone().primary_title(LINT.desc).element(
            Snippet::source(contents)
                .path(manifest_path.to_owned())
                .annotation(AnnotationKind::Primary.span(span.clone()).label(label)),
        ),
        Level::HELP
            .secondary_title(secondary_title)
            .element(Snippet::source(contents).patch(Patch::new(span.clone(), replacement)))
            .element(Level::NOTE.message(emitted_source)),
    ]
}

fn get_suggested_version_req(req: &OptVersionReq) -> Option<String> {
    use semver::Op;
    let OptVersionReq::Req(req) = req else {
        return None;
    };
    let mut has_suggestions = false;
    let mut comparators = Vec::new();

    for mut cmp in req.comparators.iter().cloned() {
        match cmp.op {
            Op::Caret | Op::GreaterEq => {
                // Only focus on comparator that has only `major` or `major.minor`
                if cmp.minor.is_some() && cmp.patch.is_some() {
                    comparators.push(cmp);
                    continue;
                } else {
                    has_suggestions = true;
                    cmp.minor.get_or_insert(0);
                    cmp.patch.get_or_insert(0);
                    comparators.push(cmp);
                }
            }
            Op::Exact | Op::Tilde | Op::Wildcard | Op::Greater | Op::Less | Op::LessEq => {
                comparators.push(cmp);
                continue;
            }
            _ => panic!("unknown comparator in `{cmp}`"),
        }
    }

    if !has_suggestions {
        return None;
    }

    // This is a lossy suggestion that
    //
    // * extra spaces are removed
    // * caret operator `^` is stripped
    let mut suggestion = String::new();

    for cmp in &comparators {
        if !suggestion.is_empty() {
            suggestion.push_str(", ");
        }
        let s = cmp.to_string();

        if cmp.op == Op::Caret {
            suggestion.push_str(s.strip_prefix('^').unwrap_or(&s));
        } else {
            suggestion.push_str(&s);
        }
    }

    Some(suggestion)
}

/// A map from parsed `Platform` to their original TOML key strings.
/// This is needed for constructing TOML key paths in diagnostics.
///
/// This is only relevant for package dependencies.
fn target_key_for_platform(manifest: &Manifest) -> HashMap<Platform, String> {
    manifest
        .normalized_toml()
        .target
        .as_ref()
        .map(|map| {
            map.keys()
                .map(|k| (k.parse().expect("already parsed"), k.clone()))
                .collect()
        })
        .unwrap_or_default()
}
