use crate::core::{Edition, Feature, Features, Manifest, MaybePackage, Package};
use crate::{CargoResult, GlobalContext};
use annotate_snippets::{AnnotationKind, Group, Level, Patch, Snippet};
use cargo_util_schemas::manifest::{ProfilePackageSpec, TomlLintLevel, TomlToolLints};
use pathdiff::diff_paths;
use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Range;
use std::path::Path;

const LINT_GROUPS: &[LintGroup] = &[TEST_DUMMY_UNSTABLE];
pub const LINTS: &[Lint] = &[BLANKET_HINT_MOSTLY_UNUSED, IM_A_TEAPOT, UNKNOWN_LINTS];

pub fn analyze_cargo_lints_table(
    pkg: &Package,
    path: &Path,
    pkg_lints: &TomlToolLints,
    ws_contents: &str,
    ws_document: &toml::Spanned<toml::de::DeTable<'static>>,
    ws_path: &Path,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let mut error_count = 0;
    let manifest = pkg.manifest();
    let manifest_path = rel_cwd_manifest_path(path, gctx);
    let ws_path = rel_cwd_manifest_path(ws_path, gctx);
    let mut unknown_lints = Vec::new();
    for lint_name in pkg_lints.keys().map(|name| name) {
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
            pkg_lints,
            manifest.edition(),
        );

        // Only run analysis on user-specified lints
        if !reason.is_user_specified() {
            continue;
        }

        // Only run this on lints that are gated by a feature
        if let Some(feature_gate) = feature_gate {
            verify_feature_enabled(
                name,
                feature_gate,
                manifest,
                &manifest_path,
                ws_contents,
                ws_document,
                &ws_path,
                &mut error_count,
                gctx,
            )?;
        }
    }

    output_unknown_lints(
        unknown_lints,
        manifest,
        &manifest_path,
        pkg_lints,
        ws_contents,
        ws_document,
        &ws_path,
        &mut error_count,
        gctx,
    )?;

    if error_count > 0 {
        Err(anyhow::anyhow!(
            "encountered {error_count} errors(s) while verifying lints",
        ))
    } else {
        Ok(())
    }
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

fn verify_feature_enabled(
    lint_name: &str,
    feature_gate: &Feature,
    manifest: &Manifest,
    manifest_path: &str,
    ws_contents: &str,
    ws_document: &toml::Spanned<toml::de::DeTable<'static>>,
    ws_path: &str,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    if !manifest.unstable_features().is_enabled(feature_gate) {
        let dash_feature_name = feature_gate.name().replace("_", "-");
        let title = format!("use of unstable lint `{}`", lint_name);
        let label = format!(
            "this is behind `{}`, which is not enabled",
            dash_feature_name
        );
        let second_title = format!("`cargo::{}` was inherited", lint_name);
        let help = format!(
            "consider adding `cargo-features = [\"{}\"]` to the top of the manifest",
            dash_feature_name
        );

        let (contents, path, span) = if let Some(span) =
            get_key_value_span(manifest.document(), &["lints", "cargo", lint_name])
        {
            (manifest.contents(), manifest_path, span)
        } else if let Some(lint_span) =
            get_key_value_span(ws_document, &["workspace", "lints", "cargo", lint_name])
        {
            (ws_contents, ws_path, lint_span)
        } else {
            panic!("could not find `cargo::{lint_name}` in `[lints]`, or `[workspace.lints]` ")
        };

        let mut report = Vec::new();
        report.push(
            Group::with_title(Level::ERROR.primary_title(title))
                .element(
                    Snippet::source(contents)
                        .path(path)
                        .annotation(AnnotationKind::Primary.span(span.key).label(label)),
                )
                .element(Level::HELP.message(help)),
        );

        if let Some(inherit_span) = get_key_value_span(manifest.document(), &["lints", "workspace"])
        {
            report.push(
                Group::with_title(Level::NOTE.secondary_title(second_title)).element(
                    Snippet::source(manifest.contents())
                        .path(manifest_path)
                        .annotation(
                            AnnotationKind::Context
                                .span(inherit_span.key.start..inherit_span.value.end),
                        ),
                ),
            );
        }

        *error_count += 1;
        gctx.shell().print_report(&report, true)?;
    }
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

/// This lint is only to be used for testing purposes
const IM_A_TEAPOT: Lint = Lint {
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
        IM_A_TEAPOT.level(pkg_lints, manifest.edition(), manifest.unstable_features());

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
        let emitted_reason = IM_A_TEAPOT.emitted_source(lint_level, reason);

        let span = get_key_value_span(manifest.document(), &["package", "im-a-teapot"]).unwrap();

        let report = &[Group::with_title(level.primary_title(IM_A_TEAPOT.desc))
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

const BLANKET_HINT_MOSTLY_UNUSED: Lint = Lint {
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
    let (lint_level, reason) = BLANKET_HINT_MOSTLY_UNUSED.level(
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
                primary_group =
                    primary_group
                        .element(Level::NOTE.message(
                            BLANKET_HINT_MOSTLY_UNUSED.emitted_source(lint_level, reason),
                        ));
            }

            // The primary group should always be first
            report.insert(0, primary_group);

            gctx.shell().print_report(&report, lint_level.force())?;
        }
    }

    Ok(())
}

const UNKNOWN_LINTS: Lint = Lint {
    name: "unknown_lints",
    desc: "unknown lint",
    groups: &[],
    default_level: LintLevel::Warn,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does
Checks for unknown lints in the `[lints.cargo]` table

### Why it is bad
- The lint name could be misspelled, leading to confusion as to why it is
  not working as expected
- The unknown lint could end up causing an error if `cargo` decides to make
  a lint with the same name in the future

### Example
```toml
[lints.cargo]
this-lint-does-not-exist = "warn"
```
"#,
    ),
};

fn output_unknown_lints(
    unknown_lints: Vec<&String>,
    manifest: &Manifest,
    manifest_path: &str,
    pkg_lints: &TomlToolLints,
    ws_contents: &str,
    ws_document: &toml::Spanned<toml::de::DeTable<'static>>,
    ws_path: &str,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) =
        UNKNOWN_LINTS.level(pkg_lints, manifest.edition(), manifest.unstable_features());
    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let level = lint_level.to_diagnostic_level();
    let mut emitted_source = None;
    for lint_name in unknown_lints {
        if lint_level.is_error() {
            *error_count += 1;
        }
        let title = format!("{}: `{lint_name}`", UNKNOWN_LINTS.desc);
        let second_title = format!("`cargo::{}` was inherited", lint_name);
        let underscore_lint_name = lint_name.replace("-", "_");
        let matching = if let Some(lint) = LINTS.iter().find(|l| l.name == underscore_lint_name) {
            Some((lint.name, "lint"))
        } else if let Some(group) = LINT_GROUPS.iter().find(|g| g.name == underscore_lint_name) {
            Some((group.name, "group"))
        } else {
            None
        };
        let help =
            matching.map(|(name, kind)| format!("there is a {kind} with a similar name: `{name}`"));

        let (contents, path, span) = if let Some(span) =
            get_key_value_span(manifest.document(), &["lints", "cargo", lint_name])
        {
            (manifest.contents(), manifest_path, span)
        } else if let Some(lint_span) =
            get_key_value_span(ws_document, &["workspace", "lints", "cargo", lint_name])
        {
            (ws_contents, ws_path, lint_span)
        } else {
            panic!("could not find `cargo::{lint_name}` in `[lints]`, or `[workspace.lints]` ")
        };

        let mut report = Vec::new();
        let mut group = Group::with_title(level.clone().primary_title(title)).element(
            Snippet::source(contents)
                .path(path)
                .annotation(AnnotationKind::Primary.span(span.key)),
        );
        if emitted_source.is_none() {
            emitted_source = Some(UNKNOWN_LINTS.emitted_source(lint_level, reason));
            group = group.element(Level::NOTE.message(emitted_source.as_ref().unwrap()));
        }
        if let Some(help) = help.as_ref() {
            group = group.element(Level::HELP.message(help));
        }
        report.push(group);

        if let Some(inherit_span) = get_key_value_span(manifest.document(), &["lints", "workspace"])
        {
            report.push(
                Group::with_title(Level::NOTE.secondary_title(second_title)).element(
                    Snippet::source(manifest.contents())
                        .path(manifest_path)
                        .annotation(
                            AnnotationKind::Context
                                .span(inherit_span.key.start..inherit_span.value.end),
                        ),
                ),
            );
        }

        gctx.shell().print_report(&report, lint_level.force())?;
    }

    Ok(())
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
        let path = snapbox::utils::current_rs!();
        let expected = std::fs::read_to_string(&path).unwrap();
        let expected = expected
            .lines()
            .filter_map(|l| {
                if l.ends_with(": Lint = Lint {") {
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
        let actual = super::LINTS
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
            "\n`LINTS` did not contain all `Lint`s found in {}\n\
            Please add the following to `LINTS`:\n\
            {}",
            path.display(),
            need_added
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
