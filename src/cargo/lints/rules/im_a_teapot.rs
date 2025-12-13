use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Feature;
use crate::core::Package;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::TEST_DUMMY_UNSTABLE;
use crate::lints::get_key_value_span;
use crate::lints::rel_cwd_manifest_path;

/// This lint is only to be used for testing purposes
pub const LINT: Lint = Lint {
    name: "im_a_teapot",
    desc: "`im_a_teapot` is specified",
    groups: &[TEST_DUMMY_UNSTABLE],
    default_level: LintLevel::Allow,
    edition_lint_opts: None,
    feature_gate: Some(Feature::test_dummy_unstable()),
    docs: None,
};

pub fn check_im_a_teapot(
    pkg: &Package,
    path: &Path,
    pkg_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest = pkg.manifest();
    let (lint_level, reason) =
        LINT.level(pkg_lints, manifest.edition(), manifest.unstable_features());

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    if manifest
        .normalized_toml()
        .package()
        .is_some_and(|p| p.im_a_teapot.is_some())
    {
        if lint_level.is_error() {
            *error_count += 1;
        }
        let level = lint_level.to_diagnostic_level();
        let manifest_path = rel_cwd_manifest_path(path, gctx);
        let emitted_reason = LINT.emitted_source(lint_level, reason);

        let span = get_key_value_span(manifest.document(), &["package", "im-a-teapot"]).unwrap();

        let report = &[Group::with_title(level.primary_title(LINT.desc))
            .element(
                Snippet::source(manifest.contents())
                    .path(&manifest_path)
                    .annotation(AnnotationKind::Primary.span(span.key.start..span.value.end)),
            )
            .element(Level::NOTE.message(&emitted_reason))];

        gctx.shell().print_report(report, lint_level.force())?;
    }
    Ok(())
}
