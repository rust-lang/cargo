use std::borrow::Cow;
use std::cmp::{Reverse, max_by_key};
use std::fmt::Display;
use std::ops::Range;
use std::path::Path;

use annotate_snippets::AnnotationKind;
use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlLintLevel;
use cargo_util_schemas::manifest::TomlToolLints;
use pathdiff::diff_paths;

use crate::core::Workspace;
use crate::core::{Edition, Feature, Features, MaybePackage, Package};
use crate::{CargoResult, GlobalContext};

pub mod rules;
pub use rules::LINTS;

pub static LINT_GROUPS: &[LintGroup] = &[
    COMPLEXITY,
    CORRECTNESS,
    NURSERY,
    PEDANTIC,
    PERF,
    RESTRICTION,
    STYLE,
    SUSPICIOUS,
    TEST_DUMMY_UNSTABLE,
];

/// Scope at which a lint runs: package-level or workspace-level.
pub enum ManifestFor<'a> {
    /// Lint runs for a specific package.
    Package(&'a Package),
    /// Lint runs for workspace-level config.
    Workspace {
        ws: &'a Workspace<'a>,
        maybe_pkg: &'a MaybePackage,
    },
}

impl ManifestFor<'_> {
    fn lint_level(&self, pkg_lints: &TomlToolLints, lint: &Lint) -> (LintLevel, LintLevelReason) {
        lint.level(pkg_lints, self.edition(), self.unstable_features())
    }

    pub fn contents(&self) -> Option<&str> {
        match self {
            ManifestFor::Package(p) => p.manifest().contents(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.contents(),
        }
    }

    pub fn document(&self) -> Option<&toml::Spanned<toml::de::DeTable<'static>>> {
        match self {
            ManifestFor::Package(p) => p.manifest().document(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.document(),
        }
    }

    pub fn edition(&self) -> Edition {
        match self {
            ManifestFor::Package(p) => p.manifest().edition(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.edition(),
        }
    }

    pub fn unstable_features(&self) -> &Features {
        match self {
            ManifestFor::Package(p) => p.manifest().unstable_features(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.unstable_features(),
        }
    }
}

impl<'a> From<&'a Package> for ManifestFor<'a> {
    fn from(value: &'a Package) -> ManifestFor<'a> {
        ManifestFor::Package(value)
    }
}

impl<'a> From<(&'a Workspace<'a>, &'a MaybePackage)> for ManifestFor<'a> {
    fn from((ws, maybe_pkg): (&'a Workspace<'a>, &'a MaybePackage)) -> ManifestFor<'a> {
        ManifestFor::Workspace { ws, maybe_pkg }
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
            &lint.primary_group.default_level,
            &lint.edition_lint_opts,
            &lint.feature_gate,
        ))
    } else if let Some(group) = LINT_GROUPS.iter().find(|g| g.name == name) {
        Some((group.name, &group.default_level, &None, &group.feature_gate))
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
        ManifestFor::Workspace { .. } => &["workspace", "lints", "cargo", lint_name][..],
    };

    let mut error = Group::with_title(Level::ERROR.primary_title(title));

    if let Some(document) = manifest.document()
        && let Some(contents) = manifest.contents()
    {
        let Some(span) = get_key_value_span(document, key_path) else {
            // This lint is handled by either package or workspace lint.
            return Ok(());
        };

        error = error.element(
            Snippet::source(contents)
                .path(manifest_path)
                .annotation(AnnotationKind::Primary.span(span.key).label(label)),
        )
    }

    let report = [error.element(Level::HELP.message(help))];

    *error_count += 1;
    gctx.shell().print_report(&report, true)?;

    Ok(())
}

#[derive(Clone)]
pub struct TomlSpan {
    pub key: Range<usize>,
    pub value: Range<usize>,
}

#[derive(Copy, Clone)]
pub enum TomlIndex<'i> {
    Key(&'i str),
    Offset(usize),
}

impl<'i> TomlIndex<'i> {
    fn as_key(&self) -> Option<&'i str> {
        match self {
            TomlIndex::Key(key) => Some(key),
            TomlIndex::Offset(_) => None,
        }
    }
}

pub trait AsIndex {
    fn as_index<'i>(&'i self) -> TomlIndex<'i>;
}

impl AsIndex for TomlIndex<'_> {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        match self {
            TomlIndex::Key(key) => TomlIndex::Key(key),
            TomlIndex::Offset(offset) => TomlIndex::Offset(*offset),
        }
    }
}

impl AsIndex for &str {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        TomlIndex::Key(self)
    }
}

impl AsIndex for usize {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        TomlIndex::Offset(*self)
    }
}

pub fn get_key_value<'doc, 'i>(
    document: &'doc toml::Spanned<toml::de::DeTable<'static>>,
    path: &[impl AsIndex],
) -> Option<(
    &'doc toml::Spanned<Cow<'doc, str>>,
    &'doc toml::Spanned<toml::de::DeValue<'static>>,
)> {
    let table = document.get_ref();
    let mut iter = path.into_iter();
    let index0 = iter.next()?.as_index();
    let key0 = index0.as_key()?;
    let (mut current_key, mut current_item) = table.get_key_value(key0)?;

    while let Some(index) = iter.next() {
        match index.as_index() {
            TomlIndex::Key(key) => {
                if let Some(table) = current_item.get_ref().as_table() {
                    (current_key, current_item) = table.get_key_value(key)?;
                } else if let Some(array) = current_item.get_ref().as_array() {
                    current_item = array.iter().find(|item| match item.get_ref() {
                        toml::de::DeValue::String(s) => s == key,
                        _ => false,
                    })?;
                } else {
                    return None;
                }
            }
            TomlIndex::Offset(offset) => {
                let array = current_item.get_ref().as_array()?;
                current_item = array.get(offset)?;
            }
        }
    }
    Some((current_key, current_item))
}

pub fn get_key_value_span<'i>(
    document: &toml::Spanned<toml::de::DeTable<'static>>,
    path: &[impl AsIndex],
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

#[derive(Clone, Debug)]
pub struct LintGroup {
    pub name: &'static str,
    pub default_level: LintLevel,
    pub desc: &'static str,
    pub feature_gate: Option<&'static Feature>,
    pub hidden: bool,
}

const COMPLEXITY: LintGroup = LintGroup {
    name: "complexity",
    desc: "code that does something simple but in a complex way",
    default_level: LintLevel::Warn,
    feature_gate: None,
    hidden: false,
};

const CORRECTNESS: LintGroup = LintGroup {
    name: "correctness",
    desc: "code that is outright wrong or useless",
    default_level: LintLevel::Deny,
    feature_gate: None,
    hidden: false,
};

const NURSERY: LintGroup = LintGroup {
    name: "nursery",
    desc: "new lints that are still under development",
    default_level: LintLevel::Allow,
    feature_gate: None,
    hidden: false,
};

const PEDANTIC: LintGroup = LintGroup {
    name: "pedantic",
    desc: "lints which are rather strict or have occasional false positives",
    default_level: LintLevel::Allow,
    feature_gate: None,
    hidden: false,
};

const PERF: LintGroup = LintGroup {
    name: "perf",
    desc: "code that can be written to run faster",
    default_level: LintLevel::Warn,
    feature_gate: None,
    hidden: false,
};

const RESTRICTION: LintGroup = LintGroup {
    name: "restriction",
    desc: "lints which prevent the use of Cargo features",
    default_level: LintLevel::Allow,
    feature_gate: None,
    hidden: false,
};

const STYLE: LintGroup = LintGroup {
    name: "style",
    desc: "code that should be written in a more idiomatic way",
    default_level: LintLevel::Warn,
    feature_gate: None,
    hidden: false,
};

const SUSPICIOUS: LintGroup = LintGroup {
    name: "suspicious",
    desc: "code that is most likely wrong or useless",
    default_level: LintLevel::Warn,
    feature_gate: None,
    hidden: false,
};

/// This lint group is only to be used for testing purposes
const TEST_DUMMY_UNSTABLE: LintGroup = LintGroup {
    name: "test_dummy_unstable",
    desc: "test_dummy_unstable is meant to only be used in tests",
    default_level: LintLevel::Allow,
    feature_gate: Some(Feature::test_dummy_unstable()),
    hidden: true,
};

#[derive(Clone, Debug)]
pub struct Lint {
    pub name: &'static str,
    pub desc: &'static str,
    pub primary_group: &'static LintGroup,
    /// The minimum supported Rust version for applying this lint
    ///
    /// Note: If the lint is on by default and did not qualify as a hard-warning before the
    /// linting system, then at earliest an MSRV of 1.78 is required as `[lints.cargo]` was a hard
    /// error before then.
    pub msrv: Option<RustVersion>,
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

        let lint_level_priority = level_priority(
            self.name,
            self.primary_group.default_level,
            self.edition_lint_opts,
            pkg_lints,
            edition,
        );

        let group_level_priority = level_priority(
            self.primary_group.name,
            self.primary_group.default_level,
            None,
            pkg_lints,
            edition,
        );

        let (_, (l, r, _)) = max_by_key(
            (self.name, lint_level_priority),
            (self.primary_group.name, group_level_priority),
            |(n, (l, _, p))| (l == &LintLevel::Forbid, *p, Reverse(*n)),
        );
        (l, r)
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
