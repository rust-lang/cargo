use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Origin;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::core::Workspace;
use crate::lints::AsIndex;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::LintLevelReason;
use crate::lints::STYLE;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

pub static LINT: &Lint = &Lint {
    name: "non_kebab_case_bins",
    desc: "binaries should have a kebab-case name",
    primary_group: &STYLE,
    msrv: None,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Detect binary names, explicit and implicit, that are not kebab-case

### Why it is bad

Kebab-case binary names is a common convention among command line tools.

### Drawbacks

It would be disruptive to existing users to change the binary name.

A binary may need to conform to externally controlled conventions which can include a different naming convention.

GUI applications may wish to choose a more user focused naming convention, like "Title Case" or "Sentence case".

### Example

```toml
[[bin]]
name = "foo_bar"
```

Should be written as:

```toml
[[bin]]
name = "foo-bar"
```
"#,
    ),
};

pub fn non_kebab_case_bins(
    ws: &Workspace<'_>,
    pkg: &Package,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        pkg.manifest().edition(),
        pkg.manifest().unstable_features(),
    );

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);

    lint_package(
        ws,
        pkg,
        &manifest_path,
        lint_level,
        reason,
        error_count,
        gctx,
    )
}

pub fn lint_package(
    ws: &Workspace<'_>,
    pkg: &Package,
    manifest_path: &str,
    lint_level: LintLevel,
    reason: LintLevelReason,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest = pkg.manifest();

    for (i, bin) in manifest.normalized_toml().bin.iter().flatten().enumerate() {
        let Some(original_name) = bin.name.as_deref() else {
            continue;
        };
        let kebab_case = heck::ToKebabCase::to_kebab_case(original_name);
        if kebab_case == original_name {
            continue;
        }

        let document = manifest.document();
        let contents = manifest.contents();
        let level = lint_level.to_diagnostic_level();
        let emitted_source = LINT.emitted_source(lint_level, reason);

        let mut primary_source = ws.target_dir().as_path_unlocked().to_owned();
        // Elide profile/platform as we don't have that context
        primary_source.push("...");
        primary_source.push("");
        let mut primary_source = primary_source.display().to_string();
        let primary_span_start = primary_source.len();
        let primary_span_end = primary_span_start + original_name.len();
        primary_source.push_str(original_name);
        primary_source.push_str(std::env::consts::EXE_SUFFIX);
        let mut primary_group =
            level
                .primary_title(LINT.desc)
                .element(Snippet::source(&primary_source).annotation(
                    AnnotationKind::Primary.span(primary_span_start..primary_span_end),
                ));
        if i == 0 {
            primary_group = primary_group.element(Level::NOTE.message(emitted_source));
        }
        let mut report = vec![primary_group];

        if let Some((i, _target)) = manifest
            .original_toml()
            .iter()
            .flat_map(|m| m.bin.iter().flatten())
            .enumerate()
            .find(|(_i, t)| t.name.as_deref() == Some(original_name))
        {
            let mut help = Group::with_title(
                Level::HELP
                    .secondary_title("to change the binary name to kebab case, convert `bin.name`"),
            );
            if let Some(document) = document
                && let Some(contents) = contents
                && let Some(span) = get_key_value_span(
                    document,
                    &["bin".as_index(), i.as_index(), "name".as_index()],
                )
            {
                help = help.element(
                    Snippet::source(contents)
                        .path(manifest_path)
                        .patch(Patch::new(span.value, format!("\"{kebab_case}\""))),
                );
            } else {
                help = help.element(Origin::path(manifest_path));
            }
            report.push(help);
        } else if is_default_main(bin.path.as_ref())
            && manifest
                .original_toml()
                .iter()
                .flat_map(|m| m.bin.iter().flatten())
                .all(|t| t.path != bin.path)
            && manifest
                .original_toml()
                .and_then(|t| t.package.as_ref())
                .map(|p| p.name.is_some())
                .unwrap_or(false)
        {
            // Showing package in case this is done before first publish to fix the problem at the
            // root
            let help_package_name =
                "to change the binary name to kebab case, convert `package.name`";
            // Including `[[bin]]` in case it is already published.
            // Preferring it over moving the file to avoid having to get into moving the
            // files it `mod`s
            let help_bin_table = "to change the binary name to kebab case, specify `bin.name`";
            if let Some(document) = document
                && let Some(contents) = contents
                && let Some(span) = get_key_value_span(document, &["package", "name"])
            {
                report.push(
                    Level::HELP.secondary_title(help_package_name).element(
                        Snippet::source(contents)
                            .path(manifest_path)
                            .patch(Patch::new(span.value, format!("\"{kebab_case}\""))),
                    ),
                );
                report.push(
                    Level::HELP.secondary_title(help_bin_table).element(
                        Snippet::source(contents)
                            .path(manifest_path)
                            .patch(Patch::new(
                                contents.len()..contents.len(),
                                format!(
                                    r#"
[[bin]]
name = "{kebab_case}"
path = "src/main.rs""#
                                ),
                            )),
                    ),
                );
            } else {
                report.push(
                    Level::HELP
                        .secondary_title(help_package_name)
                        .element(Origin::path(manifest_path)),
                );
                report.push(
                    Level::HELP
                        .secondary_title(help_bin_table)
                        .element(Origin::path(manifest_path)),
                );
            }
        } else {
            let path = bin
                .path
                .as_ref()
                .expect("normalized have a path")
                .0
                .as_path();
            let display_path = path.as_os_str().to_string_lossy();
            let end = display_path.len() - if display_path.ends_with(".rs") { 3 } else { 0 };
            let start = path
                .parent()
                .map(|p| {
                    let p = p.as_os_str().to_string_lossy();
                    // Account for trailing slash that was removed
                    p.len() + if p.is_empty() { 0 } else { 1 }
                })
                .unwrap_or(0);
            let help = Level::HELP
                .secondary_title("to change the binary name to kebab case, convert the file stem")
                .element(Snippet::source(display_path).patch(Patch::new(start..end, kebab_case)));
            report.push(help);
        }

        if lint_level.is_error() {
            *error_count += 1;
        }
        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
}

fn is_default_main(path: Option<&cargo_util_schemas::manifest::PathValue>) -> bool {
    let Some(path) = path else {
        return false;
    };
    path.0 == std::path::Path::new("src/main.rs")
}
