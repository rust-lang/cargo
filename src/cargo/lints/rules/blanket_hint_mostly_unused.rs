use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Level;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::ProfilePackageSpec;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

pub const LINT: Lint = Lint {
    name: "blanket_hint_mostly_unused",
    desc: "blanket_hint_mostly_unused lint",
    groups: &[],
    default_level: LintLevel::Warn,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks if `hint-mostly-unused` being applied to all dependencies.

### Why it is bad
`hint-mostly-unused` indicates that most of a crate's API surface will go
unused by anything depending on it; this hint can speed up the build by
attempting to minimize compilation time for items that aren't used at all.
Misapplication to crates that don't fit that criteria will slow down the build
rather than speeding it up. It should be selectively applied to dependencies
that meet these criteria. Applying it globally is always a misapplication and
will likely slow down the build.

### Example
```toml
[profile.dev.package."*"]
hint-mostly-unused = true
```

Should instead be:
```toml
[profile.dev.package.huge-mostly-unused-dependency]
hint-mostly-unused = true
```
"#,
    ),
};

pub fn blanket_hint_mostly_unused(
    maybe_pkg: &MaybePackage,
    path: &Path,
    pkg_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        pkg_lints,
        maybe_pkg.edition(),
        maybe_pkg.unstable_features(),
    );

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let level = lint_level.to_diagnostic_level();
    let manifest_path = rel_cwd_manifest_path(path, gctx);
    let mut paths = Vec::new();

    if let Some(profiles) = maybe_pkg.profiles() {
        for (profile_name, top_level_profile) in &profiles.0 {
            if let Some(true) = top_level_profile.hint_mostly_unused {
                paths.push((
                    vec!["profile", profile_name.as_str(), "hint-mostly-unused"],
                    true,
                ));
            }

            if let Some(build_override) = &top_level_profile.build_override
                && let Some(true) = build_override.hint_mostly_unused
            {
                paths.push((
                    vec![
                        "profile",
                        profile_name.as_str(),
                        "build-override",
                        "hint-mostly-unused",
                    ],
                    false,
                ));
            }

            if let Some(packages) = &top_level_profile.package
                && let Some(profile) = packages.get(&ProfilePackageSpec::All)
                && let Some(true) = profile.hint_mostly_unused
            {
                paths.push((
                    vec![
                        "profile",
                        profile_name.as_str(),
                        "package",
                        "*",
                        "hint-mostly-unused",
                    ],
                    false,
                ));
            }
        }
    }

    for (i, (path, show_per_pkg_suggestion)) in paths.iter().enumerate() {
        if lint_level.is_error() {
            *error_count += 1;
        }
        let title = "`hint-mostly-unused` is being blanket applied to all dependencies";
        let help_txt =
            "scope `hint-mostly-unused` to specific packages with a lot of unused object code";
        if let (Some(span), Some(table_span)) = (
            get_key_value_span(maybe_pkg.document(), &path),
            get_key_value_span(maybe_pkg.document(), &path[..path.len() - 1]),
        ) {
            let mut report = Vec::new();
            let mut primary_group = level.clone().primary_title(title).element(
                Snippet::source(maybe_pkg.contents())
                    .path(&manifest_path)
                    .annotation(
                        AnnotationKind::Primary.span(table_span.key.start..table_span.key.end),
                    )
                    .annotation(AnnotationKind::Context.span(span.key.start..span.value.end)),
            );

            if *show_per_pkg_suggestion {
                report.push(
                    Level::HELP.secondary_title(help_txt).element(
                        Snippet::source(maybe_pkg.contents())
                            .path(&manifest_path)
                            .patch(Patch::new(
                                table_span.key.end..table_span.key.end,
                                ".package.<pkg_name>",
                            )),
                    ),
                );
            } else {
                primary_group = primary_group.element(Level::HELP.message(help_txt));
            }

            if i == 0 {
                primary_group = primary_group
                    .element(Level::NOTE.message(LINT.emitted_source(lint_level, reason)));
            }

            // The primary group should always be first
            report.insert(0, primary_group);

            gctx.shell().print_report(&report, lint_level.force())?;
        }
    }

    Ok(())
}
