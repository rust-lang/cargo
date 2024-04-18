use crate::core::dependency::DepKind;
use crate::core::FeatureValue::Dep;
use crate::core::{Edition, FeatureValue, Package};
use crate::util::interning::InternedString;
use crate::{CargoResult, GlobalContext};
use annotate_snippets::{Level, Renderer, Snippet};
use cargo_util_schemas::manifest::{TomlLintLevel, TomlToolLints};
use pathdiff::diff_paths;
use std::collections::HashSet;
use std::fmt::Display;
use std::ops::Range;
use std::path::Path;
use toml_edit::ImDocument;

fn get_span(document: &ImDocument<String>, path: &[&str], get_value: bool) -> Option<Range<usize>> {
    let mut table = document.as_item().as_table_like().unwrap();
    let mut iter = path.into_iter().peekable();
    while let Some(key) = iter.next() {
        let (key, item) = table.get_key_value(key).unwrap();
        if iter.peek().is_none() {
            return if get_value {
                item.span()
            } else {
                let leaf_decor = key.dotted_decor();
                let leaf_prefix_span = leaf_decor.prefix().and_then(|p| p.span());
                let leaf_suffix_span = leaf_decor.suffix().and_then(|s| s.span());
                if let (Some(leaf_prefix_span), Some(leaf_suffix_span)) =
                    (leaf_prefix_span, leaf_suffix_span)
                {
                    Some(leaf_prefix_span.start..leaf_suffix_span.end)
                } else {
                    key.span()
                }
            };
        }
        if item.is_table_like() {
            table = item.as_table_like().unwrap();
        }
        if item.is_array() && iter.peek().is_some() {
            let array = item.as_array().unwrap();
            let next = iter.next().unwrap();
            return array.iter().find_map(|item| {
                if next == &item.to_string() {
                    item.span()
                } else {
                    None
                }
            });
        }
    }
    None
}

/// Gets the relative path to a manifest from the current working directory, or
/// the absolute path of the manifest if a relative path cannot be constructed
fn rel_cwd_manifest_path(path: &Path, gctx: &GlobalContext) -> String {
    diff_paths(path, gctx.cwd())
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

#[derive(Copy, Clone, Debug)]
pub struct LintGroup {
    pub name: &'static str,
    pub default_level: LintLevel,
    pub desc: &'static str,
    pub edition_lint_opts: Option<(Edition, LintLevel)>,
}

#[derive(Copy, Clone, Debug)]
pub struct Lint {
    pub name: &'static str,
    pub desc: &'static str,
    pub groups: &'static [LintGroup],
    pub default_level: LintLevel,
    pub edition_lint_opts: Option<(Edition, LintLevel)>,
}

impl Lint {
    pub fn level(&self, lints: &TomlToolLints, edition: Edition) -> LintLevel {
        let level = self
            .groups
            .iter()
            .map(|g| g.name)
            .chain(std::iter::once(self.name))
            .filter_map(|n| lints.get(n).map(|l| (n, l)))
            .max_by_key(|(n, l)| (l.priority(), std::cmp::Reverse(*n)));

        match level {
            Some((_, toml_lint)) => toml_lint.level().into(),
            None => {
                if let Some((lint_edition, lint_level)) = self.edition_lint_opts {
                    if edition >= lint_edition {
                        return lint_level;
                    }
                }
                self.default_level
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LintLevel {
    Allow,
    Warn,
    Deny,
    Forbid,
}

impl Display for LintLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintLevel::Allow => write!(f, "allow"),
            LintLevel::Warn => write!(f, "warn"),
            LintLevel::Deny => write!(f, "deny"),
            LintLevel::Forbid => write!(f, "forbid"),
        }
    }
}

impl LintLevel {
    pub fn to_diagnostic_level(self) -> Level {
        match self {
            LintLevel::Allow => Level::Note,
            LintLevel::Warn => Level::Warning,
            LintLevel::Deny => Level::Error,
            LintLevel::Forbid => Level::Error,
        }
    }
}

impl From<TomlLintLevel> for LintLevel {
    fn from(toml_lint_level: TomlLintLevel) -> LintLevel {
        match toml_lint_level {
            TomlLintLevel::Allow => LintLevel::Allow,
            TomlLintLevel::Warn => LintLevel::Warn,
            TomlLintLevel::Deny => LintLevel::Deny,
            TomlLintLevel::Forbid => LintLevel::Forbid,
        }
    }
}

/// By default, cargo will treat any optional dependency as a [feature]. As of
/// cargo 1.60, these can be disabled by declaring a feature that activates the
/// optional dependency as `dep:<name>` (see [RFC #3143]).
///
/// In the 2024 edition, `cargo` will stop exposing optional dependencies as
/// features implicitly, requiring users to add `foo = ["dep:foo"]` if they
/// still want it exposed.
///
/// For more information, see [RFC #3491]
///
/// [feature]: https://doc.rust-lang.org/cargo/reference/features.html
/// [RFC #3143]: https://rust-lang.github.io/rfcs/3143-cargo-weak-namespaced-features.html
/// [RFC #3491]: https://rust-lang.github.io/rfcs/3491-remove-implicit-features.html
const IMPLICIT_FEATURES: Lint = Lint {
    name: "implicit_features",
    desc: "implicit features for optional dependencies is deprecated and will be unavailable in the 2024 edition",
    groups: &[],
    default_level: LintLevel::Allow,
    edition_lint_opts: None,
};

pub fn check_implicit_features(
    pkg: &Package,
    path: &Path,
    lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let edition = pkg.manifest().edition();
    // In Edition 2024+, instead of creating optional features, the dependencies are unused.
    // See `UNUSED_OPTIONAL_DEPENDENCY`
    if edition >= Edition::Edition2024 {
        return Ok(());
    }

    let lint_level = IMPLICIT_FEATURES.level(lints, edition);
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let manifest = pkg.manifest();
    let activated_opt_deps = manifest
        .resolved_toml()
        .features()
        .map(|map| {
            map.values()
                .flatten()
                .filter_map(|f| match FeatureValue::new(InternedString::new(f)) {
                    Dep { dep_name } => Some(dep_name.as_str()),
                    _ => None,
                })
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let mut emitted_source = None;
    for dep in manifest.dependencies() {
        let dep_name_in_toml = dep.name_in_toml();
        if !dep.is_optional() || activated_opt_deps.contains(dep_name_in_toml.as_str()) {
            continue;
        }
        if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
            *error_count += 1;
        }
        let mut toml_path = vec![dep.kind().kind_table(), dep_name_in_toml.as_str()];
        let platform = dep.platform().map(|p| p.to_string());
        if let Some(platform) = platform.as_ref() {
            toml_path.insert(0, platform);
            toml_path.insert(0, "target");
        }
        let level = lint_level.to_diagnostic_level();
        let manifest_path = rel_cwd_manifest_path(path, gctx);
        let mut message = level.title(IMPLICIT_FEATURES.desc).snippet(
            Snippet::source(manifest.contents())
                .origin(&manifest_path)
                .annotation(level.span(get_span(manifest.document(), &toml_path, false).unwrap()))
                .fold(true),
        );
        if emitted_source.is_none() {
            emitted_source = Some(format!(
                "`cargo::{}` is set to `{lint_level}`",
                IMPLICIT_FEATURES.name
            ));
            message = message.footer(Level::Note.title(emitted_source.as_ref().unwrap()));
        }
        let renderer = Renderer::styled().term_width(
            gctx.shell()
                .err_width()
                .diagnostic_terminal_width()
                .unwrap_or(annotate_snippets::renderer::DEFAULT_TERM_WIDTH),
        );
        writeln!(gctx.shell().err(), "{}", renderer.render(message))?;
    }
    Ok(())
}

const UNUSED_OPTIONAL_DEPENDENCY: Lint = Lint {
    name: "unused_optional_dependency",
    desc: "unused optional dependency",
    groups: &[],
    default_level: LintLevel::Warn,
    edition_lint_opts: None,
};

pub fn unused_dependencies(
    pkg: &Package,
    path: &Path,
    lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let edition = pkg.manifest().edition();
    // Unused optional dependencies can only exist on edition 2024+
    if edition < Edition::Edition2024 {
        return Ok(());
    }

    let lint_level = UNUSED_OPTIONAL_DEPENDENCY.level(lints, edition);
    if lint_level == LintLevel::Allow {
        return Ok(());
    }
    let mut emitted_source = None;
    let manifest = pkg.manifest();
    let original_toml = manifest.original_toml();
    // Unused dependencies were stripped from the manifest, leaving only the used ones
    let used_dependencies = manifest
        .dependencies()
        .into_iter()
        .map(|d| d.name_in_toml().to_string())
        .collect::<HashSet<String>>();
    let mut orig_deps = vec![
        (
            original_toml.dependencies.as_ref(),
            vec![DepKind::Normal.kind_table()],
        ),
        (
            original_toml.dev_dependencies.as_ref(),
            vec![DepKind::Development.kind_table()],
        ),
        (
            original_toml.build_dependencies.as_ref(),
            vec![DepKind::Build.kind_table()],
        ),
    ];
    for (name, platform) in original_toml.target.iter().flatten() {
        orig_deps.push((
            platform.dependencies.as_ref(),
            vec!["target", name, DepKind::Normal.kind_table()],
        ));
        orig_deps.push((
            platform.dev_dependencies.as_ref(),
            vec!["target", name, DepKind::Development.kind_table()],
        ));
        orig_deps.push((
            platform.build_dependencies.as_ref(),
            vec!["target", name, DepKind::Normal.kind_table()],
        ));
    }
    for (deps, toml_path) in orig_deps {
        if let Some(deps) = deps {
            for name in deps.keys() {
                if !used_dependencies.contains(name.as_str()) {
                    if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
                        *error_count += 1;
                    }
                    let toml_path = toml_path
                        .iter()
                        .map(|s| *s)
                        .chain(std::iter::once(name.as_str()))
                        .collect::<Vec<_>>();
                    let level = lint_level.to_diagnostic_level();
                    let manifest_path = rel_cwd_manifest_path(path, gctx);

                    let mut message = level.title(UNUSED_OPTIONAL_DEPENDENCY.desc).snippet(
                        Snippet::source(manifest.contents())
                            .origin(&manifest_path)
                            .annotation(level.span(
                                get_span(manifest.document(), toml_path.as_slice(), false).unwrap(),
                            ))
                            .fold(true),
                    );
                    if emitted_source.is_none() {
                        emitted_source = Some(format!(
                            "`cargo::{}` is set to `{lint_level}`",
                            UNUSED_OPTIONAL_DEPENDENCY.name
                        ));
                        message =
                            message.footer(Level::Note.title(emitted_source.as_ref().unwrap()));
                    }
                    let help = format!(
                        "remove the dependency or activate it in a feature with `dep:{name}`"
                    );
                    message = message.footer(Level::Help.title(&help));
                    let renderer = Renderer::styled().term_width(
                        gctx.shell()
                            .err_width()
                            .diagnostic_terminal_width()
                            .unwrap_or(annotate_snippets::renderer::DEFAULT_TERM_WIDTH),
                    );
                    writeln!(gctx.shell().err(), "{}", renderer.render(message))?;
                }
            }
        }
    }
    Ok(())
}
