use crate::core::FeatureValue::Dep;
use crate::core::{Edition, FeatureValue, Package};
use crate::util::interning::InternedString;
use crate::{CargoResult, GlobalContext};
use annotate_snippets::{Level, Renderer, Snippet};
use cargo_util_schemas::manifest::{TomlLintLevel, TomlToolLints};
use pathdiff::diff_paths;
use std::collections::HashSet;
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
    desc: "warn about the use of unstable features",
    groups: &[],
    default_level: LintLevel::Allow,
    edition_lint_opts: Some((Edition::Edition2024, LintLevel::Deny)),
};

pub fn check_implicit_features(
    pkg: &Package,
    path: &Path,
    lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let lint_level = IMPLICIT_FEATURES.level(lints, pkg.manifest().edition());
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let manifest = pkg.manifest();
    let user_defined_features = manifest.resolved_toml().features();
    let features = user_defined_features.map_or(HashSet::new(), |f| {
        f.keys().map(|k| InternedString::new(&k)).collect()
    });
    // Add implicit features for optional dependencies if they weren't
    // explicitly listed anywhere.
    let explicitly_listed = user_defined_features.map_or(HashSet::new(), |f| {
        f.values()
            .flatten()
            .filter_map(|v| match FeatureValue::new(v.into()) {
                Dep { dep_name } => Some(dep_name),
                _ => None,
            })
            .collect()
    });

    for dep in manifest.dependencies() {
        let dep_name_in_toml = dep.name_in_toml();
        if !dep.is_optional()
            || features.contains(&dep_name_in_toml)
            || explicitly_listed.contains(&dep_name_in_toml)
        {
            continue;
        }
        if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
            *error_count += 1;
        }
        let level = lint_level.to_diagnostic_level();
        let manifest_path = rel_cwd_manifest_path(path, gctx);
        let message = level.title("unused optional dependency").snippet(
            Snippet::source(manifest.contents())
                .origin(&manifest_path)
                .annotation(
                    level.span(
                        get_span(
                            manifest.document(),
                            &["dependencies", &dep_name_in_toml],
                            false,
                        )
                        .unwrap(),
                    ),
                )
                .fold(true),
        );
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
