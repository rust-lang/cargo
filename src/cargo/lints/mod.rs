use crate::core::{Edition, Feature, Features, MaybePackage, Package};
use crate::{CargoResult, GlobalContext};

use annotate_snippets::AnnotationKind;
use annotate_snippets::Level;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlLintLevel;
use cargo_util_schemas::manifest::TomlToolLints;
use pathdiff::diff_paths;

use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Range;
use std::path::Path;

pub mod rules;
pub use rules::LINTS;

const LINT_GROUPS: &[LintGroup] = &[TEST_DUMMY_UNSTABLE];

/// Scope at which a lint runs: package-level or workspace-level.
pub enum ManifestFor<'a> {
    /// Lint runs for a specific package.
    Package(&'a Package),
    /// Lint runs for workspace-level config.
    Workspace(&'a MaybePackage),
}

impl ManifestFor<'_> {
    fn lint_level(&self, pkg_lints: &TomlToolLints, lint: Lint) -> (LintLevel, LintLevelReason) {
        lint.level(pkg_lints, self.edition(), self.unstable_features())
    }

    pub fn contents(&self) -> &str {
        match self {
            ManifestFor::Package(p) => p.manifest().contents(),
            ManifestFor::Workspace(p) => p.contents(),
        }
    }

    pub fn document(&self) -> &toml::Spanned<toml::de::DeTable<'static>> {
        match self {
            ManifestFor::Package(p) => p.manifest().document(),
            ManifestFor::Workspace(p) => p.document(),
        }
    }

    pub fn edition(&self) -> Edition {
        match self {
            ManifestFor::Package(p) => p.manifest().edition(),
            ManifestFor::Workspace(p) => p.edition(),
        }
    }

    pub fn unstable_features(&self) -> &Features {
        match self {
            ManifestFor::Package(p) => p.manifest().unstable_features(),
            ManifestFor::Workspace(p) => p.unstable_features(),
        }
    }
}

impl<'a> From<&'a Package> for ManifestFor<'a> {
    fn from(value: &'a Package) -> ManifestFor<'a> {
        ManifestFor::Package(value)
    }
}

impl<'a> From<&'a MaybePackage> for ManifestFor<'a> {
    fn from(value: &'a MaybePackage) -> ManifestFor<'a> {
        ManifestFor::Workspace(value)
    }
}

pub fn analyze_cargo_lints_table(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);
    let mut unknown_lints = Vec::new();
    for lint_name in cargo_lints.keys().map(|name| name) {
        let Some((name, default_level, edition_lint_opts, feature_gate)) =
            find_lint_or_group(lint_name)
        else {
            unknown_lints.push(lint_name);
            continue;
        };

        let (_, reason, _) = level_priority(
            name,
            *default_level,
            *edition_lint_opts,
            cargo_lints,
            manifest.edition(),
        );

        // Only run analysis on user-specified lints
        if !reason.is_user_specified() {
            continue;
        }

        // Only run this on lints that are gated by a feature
        if let Some(feature_gate) = feature_gate
            && !manifest.unstable_features().is_enabled(feature_gate)
        {
            report_feature_not_enabled(
                name,
                feature_gate,
                &manifest,
                &manifest_path,
                error_count,
                gctx,
            )?;
        }
    }

    rules::output_unknown_lints(
        unknown_lints,
        &manifest,
        &manifest_path,
        cargo_lints,
        error_count,
        gctx,
    )?;

    Ok(())
}

fn find_lint_or_group<'a>(
    name: &str,
) -> Option<(
    &'static str,
    &LintLevel,
    &Option<(Edition, LintLevel)>,
    &Option<&'static Feature>,
)> {
    if let Some(lint) = LINTS.iter().find(|l| l.name == name) {
        Some((
            lint.name,
            &lint.default_level,
            &lint.edition_lint_opts,
            &lint.feature_gate,
        ))
    } else if let Some(group) = LINT_GROUPS.iter().find(|g| g.name == name) {
        Some((
            group.name,
            &group.default_level,
            &group.edition_lint_opts,
            &group.feature_gate,
        ))
    } else {
        None
    }
}

fn report_feature_not_enabled(
    lint_name: &str,
    feature_gate: &Feature,
    manifest: &ManifestFor<'_>,
    manifest_path: &str,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let document = manifest.document();
    let contents = manifest.contents();
    let dash_feature_name = feature_gate.name().replace("_", "-");
    let title = format!("use of unstable lint `{}`", lint_name);
    let label = format!(
        "this is behind `{}`, which is not enabled",
        dash_feature_name
    );
    let help = format!(
        "consider adding `cargo-features = [\"{}\"]` to the top of the manifest",
        dash_feature_name
    );

    let key_path = match manifest {
        ManifestFor::Package(_) => &["lints", "cargo", lint_name][..],
        ManifestFor::Workspace(_) => &["workspace", "lints", "cargo", lint_name][..],
    };
    let Some(span) = get_key_value_span(document, key_path) else {
        // This lint is handled by either package or workspace lint.
        return Ok(());
    };

    let report = [Level::ERROR
        .primary_title(title)
        .element(
            Snippet::source(contents)
                .path(manifest_path)
                .annotation(AnnotationKind::Primary.span(span.key).label(label)),
        )
        .element(Level::HELP.message(help))];

    *error_count += 1;
    gctx.shell().print_report(&report, true)?;

    Ok(())
}

#[derive(Clone)]
pub struct TomlSpan {
    pub key: Range<usize>,
    pub value: Range<usize>,
}

pub fn get_key_value<'doc>(
    document: &'doc toml::Spanned<toml::de::DeTable<'static>>,
    path: &[&str],
) -> Option<(
    &'doc toml::Spanned<Cow<'doc, str>>,
    &'doc toml::Spanned<toml::de::DeValue<'static>>,
)> {
    let mut table = document.get_ref();
    let mut iter = path.into_iter().peekable();
    while let Some(key) = iter.next() {
        let key_s: &str = key.as_ref();
        let (key, item) = table.get_key_value(key_s)?;
        if iter.peek().is_none() {
            return Some((key, item));
        }
        if let Some(next_table) = item.get_ref().as_table() {
            table = next_table;
        }
        if iter.peek().is_some() {
            if let Some(array) = item.get_ref().as_array() {
                let next = iter.next().unwrap();
                return array.iter().find_map(|item| match item.get_ref() {
                    toml::de::DeValue::String(s) if s == next => Some((key, item)),
                    _ => None,
                });
            }
        }
    }
    None
}

pub fn get_key_value_span(
    document: &toml::Spanned<toml::de::DeTable<'static>>,
    path: &[&str],
) -> Option<TomlSpan> {
    get_key_value(document, path).map(|(k, v)| TomlSpan {
        key: k.span(),
        value: v.span(),
    })
}

/// Gets the relative path to a manifest from the current working directory, or
/// the absolute path of the manifest if a relative path cannot be constructed
pub fn rel_cwd_manifest_path(path: &Path, gctx: &GlobalContext) -> String {
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
    pub feature_gate: Option<&'static Feature>,
}

/// This lint group is only to be used for testing purposes
const TEST_DUMMY_UNSTABLE: LintGroup = LintGroup {
    name: "test_dummy_unstable",
    desc: "test_dummy_unstable is meant to only be used in tests",
    default_level: LintLevel::Allow,
    edition_lint_opts: None,
    feature_gate: Some(Feature::test_dummy_unstable()),
};

#[derive(Copy, Clone, Debug)]
pub struct Lint {
    pub name: &'static str,
    pub desc: &'static str,
    pub groups: &'static [LintGroup],
    pub default_level: LintLevel,
    pub edition_lint_opts: Option<(Edition, LintLevel)>,
    pub feature_gate: Option<&'static Feature>,
    /// This is a markdown formatted string that will be used when generating
    /// the lint documentation. If docs is `None`, the lint will not be
    /// documented.
    pub docs: Option<&'static str>,
}

impl Lint {
    pub fn level(
        &self,
        pkg_lints: &TomlToolLints,
        edition: Edition,
        unstable_features: &Features,
    ) -> (LintLevel, LintLevelReason) {
        // We should return `Allow` if a lint is behind a feature, but it is
        // not enabled, that way the lint does not run.
        if self
            .feature_gate
            .is_some_and(|f| !unstable_features.is_enabled(f))
        {
            return (LintLevel::Allow, LintLevelReason::Default);
        }

        self.groups
            .iter()
            .map(|g| {
                (
                    g.name,
                    level_priority(
                        g.name,
                        g.default_level,
                        g.edition_lint_opts,
                        pkg_lints,
                        edition,
                    ),
                )
            })
            .chain(std::iter::once((
                self.name,
                level_priority(
                    self.name,
                    self.default_level,
                    self.edition_lint_opts,
                    pkg_lints,
                    edition,
                ),
            )))
            .max_by_key(|(n, (l, _, p))| (l == &LintLevel::Forbid, *p, std::cmp::Reverse(*n)))
            .map(|(_, (l, r, _))| (l, r))
            .unwrap()
    }

    fn emitted_source(&self, lint_level: LintLevel, reason: LintLevelReason) -> String {
        format!("`cargo::{}` is set to `{lint_level}` {reason}", self.name,)
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
    pub fn is_error(&self) -> bool {
        self == &LintLevel::Forbid || self == &LintLevel::Deny
    }

    pub fn to_diagnostic_level(self) -> Level<'static> {
        match self {
            LintLevel::Allow => unreachable!("allow does not map to a diagnostic level"),
            LintLevel::Warn => Level::WARNING,
            LintLevel::Deny => Level::ERROR,
            LintLevel::Forbid => Level::ERROR,
        }
    }

    fn force(self) -> bool {
        match self {
            Self::Allow => false,
            Self::Warn => true,
            Self::Deny => true,
            Self::Forbid => true,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LintLevelReason {
    Default,
    Edition(Edition),
    Package,
}

impl Display for LintLevelReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintLevelReason::Default => write!(f, "by default"),
            LintLevelReason::Edition(edition) => write!(f, "in edition {}", edition),
            LintLevelReason::Package => write!(f, "in `[lints]`"),
        }
    }
}

impl LintLevelReason {
    fn is_user_specified(&self) -> bool {
        match self {
            LintLevelReason::Default => false,
            LintLevelReason::Edition(_) => false,
            LintLevelReason::Package => true,
        }
    }
}

fn level_priority(
    name: &str,
    default_level: LintLevel,
    edition_lint_opts: Option<(Edition, LintLevel)>,
    pkg_lints: &TomlToolLints,
    edition: Edition,
) -> (LintLevel, LintLevelReason, i8) {
    let (unspecified_level, reason) = if let Some(level) = edition_lint_opts
        .filter(|(e, _)| edition >= *e)
        .map(|(_, l)| l)
    {
        (level, LintLevelReason::Edition(edition))
    } else {
        (default_level, LintLevelReason::Default)
    };

    // Don't allow the group to be overridden if the level is `Forbid`
    if unspecified_level == LintLevel::Forbid {
        return (unspecified_level, reason, 0);
    }

    if let Some(defined_level) = pkg_lints.get(name) {
        (
            defined_level.level().into(),
            LintLevelReason::Package,
            defined_level.priority(),
        )
    } else {
        (unspecified_level, reason, 0)
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use snapbox::ToDebug;
    use std::collections::HashSet;

    #[test]
    fn ensure_sorted_lints() {
        // This will be printed out if the fields are not sorted.
        let location = std::panic::Location::caller();
        println!("\nTo fix this test, sort `LINTS` in {}\n", location.file(),);

        let actual = super::LINTS
            .iter()
            .map(|l| l.name.to_uppercase())
            .collect::<Vec<_>>();

        let mut expected = actual.clone();
        expected.sort();
        snapbox::assert_data_eq!(actual.to_debug(), expected.to_debug());
    }

    #[test]
    fn ensure_sorted_lint_groups() {
        // This will be printed out if the fields are not sorted.
        let location = std::panic::Location::caller();
        println!(
            "\nTo fix this test, sort `LINT_GROUPS` in {}\n",
            location.file(),
        );
        let actual = super::LINT_GROUPS
            .iter()
            .map(|l| l.name.to_uppercase())
            .collect::<Vec<_>>();

        let mut expected = actual.clone();
        expected.sort();
        snapbox::assert_data_eq!(actual.to_debug(), expected.to_debug());
    }

    #[test]
    fn ensure_updated_lints() {
        let dir = snapbox::utils::current_dir!().join("rules");
        let mut expected = HashSet::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.ends_with("mod.rs") {
                continue;
            }
            let lint_name = path.file_stem().unwrap().to_string_lossy();
            assert!(expected.insert(lint_name.into()), "duplicate lint found");
        }

        let actual = super::LINTS
            .iter()
            .map(|l| l.name.to_string())
            .collect::<HashSet<_>>();
        let diff = expected.difference(&actual).sorted().collect::<Vec<_>>();

        let mut need_added = String::new();
        for name in &diff {
            need_added.push_str(&format!("{name}\n"));
        }
        assert!(
            diff.is_empty(),
            "\n`LINTS` did not contain all `Lint`s found in {}\n\
            Please add the following to `LINTS`:\n\
            {need_added}",
            dir.display(),
        );
    }

    #[test]
    fn ensure_updated_lint_groups() {
        let path = snapbox::utils::current_rs!();
        let expected = std::fs::read_to_string(&path).unwrap();
        let expected = expected
            .lines()
            .filter_map(|l| {
                if l.ends_with(": LintGroup = LintGroup {") {
                    Some(
                        l.chars()
                            .skip(6)
                            .take_while(|c| *c != ':')
                            .collect::<String>(),
                    )
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();
        let actual = super::LINT_GROUPS
            .iter()
            .map(|l| l.name.to_uppercase())
            .collect::<HashSet<_>>();
        let diff = expected.difference(&actual).sorted().collect::<Vec<_>>();

        let mut need_added = String::new();
        for name in &diff {
            need_added.push_str(&format!("{}\n", name));
        }
        assert!(
            diff.is_empty(),
            "\n`LINT_GROUPS` did not contain all `LintGroup`s found in {}\n\
            Please add the following to `LINT_GROUPS`:\n\
            {}",
            path.display(),
            need_added
        );
    }
}
