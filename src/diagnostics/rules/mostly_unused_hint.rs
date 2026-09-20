use std::path::Path;

use cargo_util_schemas::manifest::TomlProfile;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Origin;
use cargo_util_terminal::report::Snippet;
use tracing::instrument;

use crate::CargoResult;
use crate::GlobalContext;
use crate::diagnostics::ScopedDiagnosticStats;
use crate::diagnostics::TomlSpan;
use crate::diagnostics::get_key_value_span;
use crate::diagnostics::workspace_rel_path;
use crate::workspace::MaybePackage;
use crate::workspace::Package;
use crate::workspace::Workspace;

const HELP: &str = "pass `-Zprofile-hint-mostly-unused` to enable it";
const PROFILE_TITLE: &str = "ignoring `hint-mostly-unused` profile option";
const HINT_KEY: &str = "hint-mostly-unused";

/// Reports a `hints.mostly-unused` that is not a boolean, or that is ignored without
/// `-Zprofile-hint-mostly-unused`.
#[instrument(skip_all)]
pub(crate) fn diagnose_package(
    ws: &Workspace<'_>,
    pkg: &Package,
    path: &Path,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let Some(value) = pkg.hints().and_then(|hints| hints.mostly_unused.as_ref()) else {
        return Ok(());
    };
    let manifest_path = workspace_rel_path(ws, path);
    let group = match value {
        // Profiles are not resolved yet, so this also fires when a profile disables the hint for
        // the package.
        toml::Value::Boolean(true) if !gctx.cli_unstable().profile_hint_mostly_unused => {
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
            group.element(Level::HELP.message(HELP))
        }
        toml::Value::Boolean(_) => return Ok(()),
        value => {
            let title = format!(
                "ignoring unsupported value type ({}) for `hints.mostly-unused`",
                value.type_str()
            );
            let group = Group::with_title(Level::WARNING.primary_title(title));
            match hint_span(pkg) {
                Some((contents, span)) => group.element(
                    Snippet::source(contents).path(&manifest_path).annotation(
                        AnnotationKind::Primary
                            .span(span.value)
                            .label("expected a boolean"),
                    ),
                ),
                None => group.element(Origin::path(&manifest_path)),
            }
        }
    };
    pkg_stats.record_warning();
    gctx.shell().print_report(&[group], false)?;
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

/// Reports each `hint-mostly-unused = true` in the root manifest's profiles that is ignored
/// without `-Zprofile-hint-mostly-unused`. Config profiles are checked where they are loaded,
/// see [`crate::workspace::profiles`].
#[instrument(skip_all)]
pub(crate) fn diagnose_workspace(
    ws: &Workspace<'_>,
    maybe_pkg: &MaybePackage,
    path: &Path,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    if gctx.cli_unstable().profile_hint_mostly_unused {
        return Ok(());
    }
    let Some(profiles) = maybe_pkg.profiles() else {
        return Ok(());
    };
    let manifest_path = workspace_rel_path(ws, path);

    // Every profile is checked, as the parse pass does not know which one is built.
    for (name, profile) in profiles.get_all() {
        for key in enabled_keys(name.as_str(), profile) {
            let group = Group::with_title(Level::WARNING.primary_title(PROFILE_TITLE));
            let group = if let Some(contents) = maybe_pkg.contents()
                && let Some(document) = maybe_pkg.document()
                && let Some(table_span) = get_key_value_span(document, &key[..key.len() - 1])
                && let Some(span) = get_key_value_span(document, &key)
            {
                group.element(
                    Snippet::source(contents)
                        .path(&manifest_path)
                        .annotation(AnnotationKind::Primary.span(span.key.start..span.value.end))
                        .annotation(AnnotationKind::Context.span(table_span.key)),
                )
            } else {
                group.element(Origin::path(&manifest_path))
            };
            let group = group.element(Level::HELP.message(HELP));
            pkg_stats.record_warning();
            gctx.shell().print_report(&[group], false)?;
        }
    }
    Ok(())
}

/// The key path of each `hint-mostly-unused = true` under `[profile.<name>]`, whether in the
/// profile itself, its `build-override`, or one of its `package` entries.
fn enabled_keys(name: &str, profile: &TomlProfile) -> Vec<Vec<String>> {
    let mut candidates = vec![(profile, vec![])];
    if let Some(build_override) = &profile.build_override {
        candidates.push((build_override, vec!["build-override".to_owned()]));
    }
    for (spec, package) in profile.package.iter().flatten() {
        // The manifest key is not kept, only the parsed spec, which `to_string` normalizes. A
        // legacy key like `"foo:1.0.0"` thus finds no span, and the warning has no snippet.
        candidates.push((package, vec!["package".to_owned(), spec.to_string()]));
    }
    let mut keys = Vec::new();
    for (sub_profile, suffix) in candidates {
        if sub_profile.hint_mostly_unused == Some(true) {
            let mut key = vec!["profile".to_owned(), name.to_owned()];
            key.extend(suffix);
            key.push(HINT_KEY.to_owned());
            keys.push(key);
        }
    }
    keys
}
