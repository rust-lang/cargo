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
use crate::util::data_structures::HashSet;
use crate::workspace::Package;

const HELP: &str = "pass `-Zprofile-hint-mostly-unused` to enable it";

#[derive(PartialEq, Eq, Hash)]
enum DiagnosticKind {
    UnsupportedHintType,
    IgnoredProfileOption,
    IgnoredPackageHint,
}

/// Emits each diagnostic for `hints.mostly-unused` and the `hint-mostly-unused` profile option once
/// per package selected for compilation.
#[tracing::instrument(skip_all)]
pub(crate) fn diagnose(bcx: &BuildContext<'_, '_>) -> CargoResult<()> {
    let gctx = bcx.gctx;
    let mut units = bcx
        .unit_graph
        .keys()
        .filter(|unit| !unit.skip_non_compile_time_dep)
        .collect::<Vec<_>>();
    units.sort();
    let mut emitted = HashSet::default();

    for unit in units {
        let pkg = &unit.pkg;
        let pkg_id = pkg.package_id();
        let gate_enabled = gctx.cli_unstable().profile_hint_mostly_unused;
        let profile_hint = unit.profile.hint_mostly_unused;

        // Profile options come from the workspace itself, so they are reported even for
        // non-local packages.
        if !gate_enabled
            && profile_hint == Some(true)
            && emitted.insert((pkg_id, DiagnosticKind::IgnoredProfileOption))
        {
            let title = format!(
                "ignoring `hint-mostly-unused` profile option for `{}@{}`",
                pkg.name(),
                pkg.version()
            );
            let group = Group::with_title(Level::WARNING.primary_title(title));
            let group = group.element(Level::HELP.message(HELP));
            gctx.shell().print_report(&[group], false)?;
        }

        if !unit.show_warnings(gctx) {
            continue;
        }

        let manifest_path = workspace_rel_path(bcx.ws, pkg.manifest_path());
        let hint_value = pkg.hints().and_then(|hints| hints.mostly_unused.as_ref());
        let pkg_hint = match hint_value {
            None => None,
            Some(toml::Value::Boolean(b)) => Some(*b),
            Some(value) => {
                if emitted.insert((pkg_id, DiagnosticKind::UnsupportedHintType)) {
                    let title = format!(
                        "ignoring unsupported value type ({}) for `hints.mostly-unused`",
                        value.type_str()
                    );
                    let group = Group::with_title(Level::WARNING.primary_title(title));
                    let group = match hint_span(pkg) {
                        Some((contents, span)) => group.element(
                            Snippet::source(contents).path(&manifest_path).annotation(
                                AnnotationKind::Primary
                                    .span(span.value)
                                    .label("expected a boolean"),
                            ),
                        ),
                        None => group.element(Origin::path(&manifest_path)),
                    };
                    gctx.shell().print_report(&[group], false)?;
                }
                None
            }
        };

        if gate_enabled
            || profile_hint.is_some()
            || pkg_hint != Some(true)
            || !emitted.insert((pkg_id, DiagnosticKind::IgnoredPackageHint))
        {
            continue;
        }
        let group =
            Group::with_title(Level::WARNING.primary_title("ignoring `hints.mostly-unused`"));
        let group = match hint_span(pkg) {
            Some((contents, span)) => group.element(
                Snippet::source(contents)
                    .path(&manifest_path)
                    .annotation(AnnotationKind::Primary.span(span.key.start..span.value.end)),
            ),
            None => group.element(Origin::path(&manifest_path)),
        };
        let group = group.element(Level::HELP.message(HELP));
        gctx.shell().print_report(&[group], false)?;
    }

    Ok(())
}

/// Locates `hints.mostly-unused` in the package's original manifest, if its source is available.
fn hint_span(pkg: &Package) -> Option<(&str, TomlSpan)> {
    let manifest = pkg.manifest();
    let contents = manifest.contents()?;
    let document = manifest.document()?;
    let span = get_key_value_span(document, &["hints", "mostly-unused"])?;
    Some((contents, span))
}
