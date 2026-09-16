use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Snippet;

use crate::CargoResult;
use crate::compiler::BuildContext;
use crate::diagnostics::TomlSpan;
use crate::diagnostics::get_key_value_span;
use crate::diagnostics::workspace_rel_path;
use crate::workspace::Package;
use crate::workspace::profiles::{MinOptLevelHintError, parse_min_opt_level_hint};

/// Emits diagnostics for `hints.min-opt-level` once per package selected for compilation.
#[tracing::instrument(skip_all)]
pub(crate) fn diagnose(bcx: &BuildContext<'_, '_>) -> CargoResult<()> {
    let gctx = bcx.gctx;
    let mut packages = bcx
        .unit_graph
        .keys()
        .filter(|unit| !unit.skip_non_compile_time_dep && unit.show_warnings(gctx))
        .map(|unit| unit.pkg.clone())
        .collect::<Vec<_>>();
    packages.sort_by_key(|pkg| pkg.package_id());
    packages.dedup_by_key(|pkg| pkg.package_id());

    for pkg in packages {
        let manifest_path = workspace_rel_path(bcx.ws, pkg.manifest_path());
        let min_opt_level = match parse_min_opt_level_hint(
            pkg.hints().and_then(|hints| hints.min_opt_level.as_ref()),
        ) {
            Ok(level) => level,
            Err(err) => {
                let (title, label) = match err {
                    MinOptLevelHintError::OutOfRange(level) => (
                        format!("ignoring unsupported value ({level}) for `hints.min-opt-level`"),
                        "expected an integer from 0 to 3",
                    ),
                    MinOptLevelHintError::WrongType(value_type) => (
                        format!(
                            "ignoring unsupported value type ({value_type}) for `hints.min-opt-level`"
                        ),
                        "expected an integer",
                    ),
                };
                let group = Group::with_title(Level::WARNING.primary_title(title));
                let group = match hint_span(&pkg) {
                    Some((contents, span)) => group.element(
                        Snippet::source(contents)
                            .path(&manifest_path)
                            .annotation(AnnotationKind::Primary.span(span.value).label(label)),
                    ),
                    None => group.element(Origin::path(&manifest_path)),
                };
                gctx.shell().print_report(&[group], false)?;
                None
            }
        };

        if matches!(min_opt_level, Some(1..=3)) && !gctx.cli_unstable().hint_min_opt_level {
            let group =
                Group::with_title(Level::WARNING.primary_title("ignoring `hints.min-opt-level`"));
            let group = match hint_span(&pkg) {
                Some((contents, span)) => group.element(
                    Snippet::source(contents)
                        .path(&manifest_path)
                        .annotation(AnnotationKind::Primary.span(span.key.start..span.value.end)),
                ),
                None => group.element(Origin::path(&manifest_path)),
            };
            let group =
                group.element(Level::HELP.message("pass `-Zhint-min-opt-level` to enable it"));
            gctx.shell().print_report(&[group], false)?;
        }
    }

    Ok(())
}

/// Locates `hints.min-opt-level` in the package's original manifest, if its source is available.
fn hint_span(pkg: &Package) -> Option<(&str, TomlSpan)> {
    let manifest = pkg.manifest();
    let contents = manifest.contents()?;
    let document = manifest.document()?;
    let span = get_key_value_span(document, &["hints", "min-opt-level"])?;
    Some((contents, span))
}
