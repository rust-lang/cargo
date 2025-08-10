use crate::core::compiler::{CompileKind, TargetInfo};
use crate::core::{Edition, Feature, Features, Manifest, Package, Workspace};
use crate::{CargoResult, GlobalContext};
use annotate_snippets::{Level, Snippet};
use cargo_platform::{Cfg, ExpectedValues, Platform};
use cargo_util_schemas::manifest::{TomlLintLevel, TomlToolLints};
use pathdiff::diff_paths;
use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Range;
use std::path::Path;

const LINT_GROUPS: &[LintGroup] = &[TEST_DUMMY_UNSTABLE];
pub const LINTS: &[Lint] = &[IM_A_TEAPOT, UNEXPECTED_CFGS, UNKNOWN_LINTS];

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

        let message = if let Some(span) =
            get_span(manifest.document(), &["lints", "cargo", lint_name], false)
        {
            Level::Error
                .title(&title)
                .snippet(
                    Snippet::source(manifest.contents())
                        .origin(&manifest_path)
                        .annotation(Level::Error.span(span).label(&label))
                        .fold(true),
                )
                .footer(Level::Help.title(&help))
        } else {
            let lint_span = get_span(
                ws_document,
                &["workspace", "lints", "cargo", lint_name],
                false,
            )
            .unwrap_or_else(|| {
                panic!("could not find `cargo::{lint_name}` in `[lints]`, or `[workspace.lints]` ")
            });

            let inherited_note = if let (Some(inherit_span_key), Some(inherit_span_value)) = (
                get_span(manifest.document(), &["lints", "workspace"], false),
                get_span(manifest.document(), &["lints", "workspace"], true),
            ) {
                Level::Note.title(&second_title).snippet(
                    Snippet::source(manifest.contents())
                        .origin(&manifest_path)
                        .annotation(
                            Level::Note.span(inherit_span_key.start..inherit_span_value.end),
                        )
                        .fold(true),
                )
            } else {
                Level::Note.title(&second_title)
            };

            Level::Error
                .title(&title)
                .snippet(
                    Snippet::source(ws_contents)
                        .origin(&ws_path)
                        .annotation(Level::Error.span(lint_span).label(&label))
                        .fold(true),
                )
                .footer(inherited_note)
                .footer(Level::Help.title(&help))
        };

        *error_count += 1;
        gctx.shell().print_message(message)?;
    }
    Ok(())
}

pub fn get_span(
    document: &toml::Spanned<toml::de::DeTable<'static>>,
    path: &[&str],
    get_value: bool,
) -> Option<Range<usize>> {
    let mut table = document.get_ref();
    let mut iter = path.into_iter().peekable();
    while let Some(key) = iter.next() {
        let key_s: &str = key.as_ref();
        let (key, item) = table.get_key_value(key_s)?;
        if iter.peek().is_none() {
            return if get_value {
                Some(item.span())
            } else {
                Some(key.span())
            };
        }
        if let Some(next_table) = item.get_ref().as_table() {
            table = next_table;
        }
        if iter.peek().is_some() {
            if let Some(array) = item.get_ref().as_array() {
                let next = iter.next().unwrap();
                return array.iter().find_map(|item| match item.get_ref() {
                    toml::de::DeValue::String(s) if s == next => Some(item.span()),
                    _ => None,
                });
            }
        }
    }
    None
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
            LintLevel::Allow => unreachable!("allow does not map to a diagnostic level"),
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
        if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
            *error_count += 1;
        }
        let level = lint_level.to_diagnostic_level();
        let manifest_path = rel_cwd_manifest_path(path, gctx);
        let emitted_reason = format!(
            "`cargo::{}` is set to `{lint_level}` {reason}",
            IM_A_TEAPOT.name
        );

        let key_span = get_span(manifest.document(), &["package", "im-a-teapot"], false).unwrap();
        let value_span = get_span(manifest.document(), &["package", "im-a-teapot"], true).unwrap();
        let message = level
            .title(IM_A_TEAPOT.desc)
            .snippet(
                Snippet::source(manifest.contents())
                    .origin(&manifest_path)
                    .annotation(level.span(key_span.start..value_span.end))
                    .fold(true),
            )
            .footer(Level::Note.title(&emitted_reason));

        gctx.shell().print_message(message)?;
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
        if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
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

        let mut message = if let Some(span) =
            get_span(manifest.document(), &["lints", "cargo", lint_name], false)
        {
            level.title(&title).snippet(
                Snippet::source(manifest.contents())
                    .origin(&manifest_path)
                    .annotation(Level::Error.span(span))
                    .fold(true),
            )
        } else {
            let lint_span = get_span(
                ws_document,
                &["workspace", "lints", "cargo", lint_name],
                false,
            )
            .unwrap_or_else(|| {
                panic!("could not find `cargo::{lint_name}` in `[lints]`, or `[workspace.lints]` ")
            });

            let inherited_note = if let (Some(inherit_span_key), Some(inherit_span_value)) = (
                get_span(manifest.document(), &["lints", "workspace"], false),
                get_span(manifest.document(), &["lints", "workspace"], true),
            ) {
                Level::Note.title(&second_title).snippet(
                    Snippet::source(manifest.contents())
                        .origin(&manifest_path)
                        .annotation(
                            Level::Note.span(inherit_span_key.start..inherit_span_value.end),
                        )
                        .fold(true),
                )
            } else {
                Level::Note.title(&second_title)
            };

            level
                .title(&title)
                .snippet(
                    Snippet::source(ws_contents)
                        .origin(&ws_path)
                        .annotation(Level::Error.span(lint_span))
                        .fold(true),
                )
                .footer(inherited_note)
        };

        if emitted_source.is_none() {
            emitted_source = Some(format!(
                "`cargo::{}` is set to `{lint_level}` {reason}",
                UNKNOWN_LINTS.name
            ));
            message = message.footer(Level::Note.title(emitted_source.as_ref().unwrap()));
        }

        if let Some(help) = help.as_ref() {
            message = message.footer(Level::Help.title(help));
        }

        gctx.shell().print_message(message)?;
    }

    Ok(())
}

const UNEXPECTED_CFGS: Lint = Lint {
    name: "unexpected_cfgs",
    desc: "lint on unexpected target cfgs",
    groups: &[],
    default_level: LintLevel::Allow,
    edition_lint_opts: None,
    feature_gate: Some(Feature::check_target_cfgs()),
    docs: Some(
        r#"
### What it does
Checks for unexpected cfgs in `[target.'cfg(...)']`

### Why it is bad
The lint helps with verifying that the crate is correctly handling conditional
compilation for different target platforms. It ensures that the cfg settings are
consistent between what is intended and what is used, helping to
catch potential bugs or errors early in the development process.

### Example
```toml
[lints.cargo]
unexpected_cfgs = "warn"
```
"#,
    ),
};

pub fn unexpected_target_cfgs(
    ws: &Workspace<'_>,
    pkg: &Package,
    path: &Path,
    pkg_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest = pkg.manifest();

    let (lint_level, _lint_reason) =
        UNEXPECTED_CFGS.level(pkg_lints, manifest.edition(), manifest.unstable_features());

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let rustc = gctx.load_global_rustc(Some(ws))?;
    // FIXME: While it doesn't doesn't really matter for `--print=check-cfg`, we should
    // still pass the actual requested targets instead of an empty array so that the
    // target info can always be de-duplicated.
    // FIXME: Move the target info creation before any linting is done and re-use it for
    // all the packages.
    let compile_kinds = CompileKind::from_requested_targets(gctx, &[])?;
    let target_info = TargetInfo::new(gctx, &compile_kinds, &rustc, CompileKind::Host)?;

    let Some(global_check_cfg) = target_info.check_cfg() else {
        return Ok(());
    };

    // If we have extra `--check-cfg` args comming from the lints config, we need to
    // refetch the `--print=check-cfg` with those extra args.
    let lint_rustflags = pkg.manifest().lint_rustflags();
    let check_cfg = if lint_rustflags.iter().any(|a| a == "--check-cfg") {
        Some(target_info.check_cfg_with_extra_args(gctx, &rustc, lint_rustflags)?)
    } else {
        None
    };
    let check_cfg = check_cfg.as_ref().unwrap_or(&global_check_cfg);

    if !check_cfg.exhaustive {
        return Ok(());
    }

    for dep in pkg.summary().dependencies() {
        let Some(platform) = dep.platform() else {
            continue;
        };
        let Platform::Cfg(cfg_expr) = platform else {
            continue;
        };

        cfg_expr.walk(|cfg| -> CargoResult<()> {
            let (name, value) = match cfg {
                Cfg::Name(name) => (name, None),
                Cfg::KeyPair(name, value) => (name, Some(value.to_string())),
            };

            match check_cfg.expecteds.get(name.as_str()) {
                Some(ExpectedValues::Some(values)) if !values.contains(&value) => {
                    let level = lint_level.to_diagnostic_level();
                    if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
                        *error_count += 1;
                    }

                    let value = match value {
                        Some(value) => Cow::from(format!("`{value}`")),
                        None => Cow::from("(none)"),
                    };

                    // Retrieving the span can fail if our pretty-priting doesn't match what the
                    // user wrote: `unix="foo"` and `unix = "foo"`, for that reason we don't fail
                    // and just produce a slightly worth diagnostic.
                    if let Some(span) = get_span(manifest.document(), &["target", &*format!("cfg({cfg_expr})")], false) {
                        let manifest_path = rel_cwd_manifest_path(path, gctx);
                        let title = format!(
                            "unexpected `cfg` condition value: {value} for `{cfg}`"
                        );
                        let diag = level.title(&*title).snippet(
                            Snippet::source(manifest.contents())
                                .origin(&manifest_path)
                                .annotation(level.span(span))
                                .fold(true)
                        );
                        gctx.shell().print_message(diag)?;
                    } else {
                        let title = format!(
                            "unexpected `cfg` condition value: {value} for `{cfg}` in `[target.'cfg({cfg_expr})']`"
                        );
                        let help_where = format!("occurred in `{path}`", path = path.display());
                        let diag = level.title(&*title).footer(Level::Help.title(&*help_where));
                        gctx.shell().print_message(diag)?;
                    }
                }
                None => {
                    let level = lint_level.to_diagnostic_level();
                    if lint_level == LintLevel::Forbid || lint_level == LintLevel::Deny {
                        *error_count += 1;
                    }

                    let for_cfg = match value {
                        Some(_value) => Cow::from(format!(" for `{cfg}`")),
                        None => Cow::from(""),
                    };

                    // Retrieving the span can fail if our pretty-priting doesn't match what the
                    // user wrote: `unix="foo"` and `unix = "foo"`, for that reason we don't fail
                    // and just produce a slightly worth diagnostic.
                    if let Some(span) = get_span(manifest.document(), &["target", &*format!("cfg({cfg_expr})")], false) {
                        let manifest_path = rel_cwd_manifest_path(path, gctx);
                        let title = format!(
                            "unexpected `cfg` condition name: {name}{for_cfg}"
                        );
                        let diag = level.title(&*title).snippet(
                            Snippet::source(manifest.contents())
                                .origin(&manifest_path)
                                .annotation(level.span(span))
                                .fold(true)
                        );
                        gctx.shell().print_message(diag)?;
                    } else {
                        let title = format!(
                            "unexpected `cfg` condition name: {name}{for_cfg} in `[target.'cfg({cfg_expr})']`"
                        );
                        let help_where = format!("occurred in `{path}`", path = path.display());
                        let diag = level.title(&*title).footer(Level::Help.title(&*help_where));
                        gctx.shell().print_message(diag)?;
                    }
                }
                _ => { /* not unexpected */ }
            }
            Ok(())
        })?;
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
