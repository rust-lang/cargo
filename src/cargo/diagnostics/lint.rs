use std::cmp::{Reverse, max_by_key};
use std::fmt::Display;

use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlLintLevel;
use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::Level;

use crate::core::{Feature, Features};

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
        pkg_rust_version: Option<&RustVersion>,
        unstable_features: &Features,
    ) -> (LintLevel, LintLevelSource) {
        // We should return `Allow` if a lint is behind a feature, but it is
        // not enabled, that way the lint does not run.
        if self
            .feature_gate
            .is_some_and(|f| !unstable_features.is_enabled(f))
        {
            return (LintLevel::Allow, LintLevelSource::Default);
        }

        if let (Some(msrv), Some(pkg_rust_version)) = (&self.msrv, pkg_rust_version) {
            let pkg_rust_version = pkg_rust_version.to_partial();
            if !msrv.is_compatible_with(&pkg_rust_version) {
                return (LintLevel::Allow, LintLevelSource::Default);
            }
        }

        let lint_level_priority =
            level_priority(self.name, self.primary_group.default_level, pkg_lints);

        let group_level_priority = level_priority(
            self.primary_group.name,
            self.primary_group.default_level,
            pkg_lints,
        );

        let (_, (l, s, _)) = max_by_key(
            (self.name, lint_level_priority),
            (self.primary_group.name, group_level_priority),
            |(n, (l, s, p))| {
                (
                    l == &LintLevel::Forbid,
                    *s != LintLevelSource::Default,
                    *p,
                    Reverse(*n),
                )
            },
        );
        (l, s)
    }

    pub fn emitted_source(&self, lint_level: LintLevel, source: LintLevelSource) -> String {
        format!("`cargo::{}` is set to `{lint_level}` {source}", self.name,)
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
    pub fn is_warn(&self) -> bool {
        self == &LintLevel::Warn
    }

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

    pub fn force(self) -> bool {
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
pub enum LintLevelSource {
    Default,
    Package,
}

impl Display for LintLevelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintLevelSource::Default => write!(f, "by default"),
            LintLevelSource::Package => write!(f, "in `[lints]`"),
        }
    }
}

impl LintLevelSource {
    pub(crate) fn is_user_specified(&self) -> bool {
        match self {
            LintLevelSource::Default => false,
            LintLevelSource::Package => true,
        }
    }
}

pub(crate) fn level_priority(
    name: &str,
    default_level: LintLevel,
    pkg_lints: &TomlToolLints,
) -> (LintLevel, LintLevelSource, i8) {
    if let Some(defined_level) = pkg_lints.get(name) {
        (
            defined_level.level().into(),
            LintLevelSource::Package,
            defined_level.priority(),
        )
    } else {
        (default_level, LintLevelSource::Default, 0)
    }
}

#[derive(Clone, Debug)]
pub struct LintGroup {
    pub name: &'static str,
    pub default_level: LintLevel,
    pub desc: &'static str,
    pub feature_gate: Option<&'static Feature>,
    pub hidden: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: LintGroup = LintGroup {
        name: "style",
        desc: "code that should be written in a more idiomatic way",
        default_level: LintLevel::Warn,
        feature_gate: None,
        hidden: false,
    };

    fn test_lint(name: &'static str, group: &'static LintGroup) -> Lint {
        Lint {
            name,
            desc: "test lint",
            primary_group: group,
            msrv: None,
            feature_gate: None,
            docs: None,
        }
    }

    #[test]
    fn lint_level_prefers_user_specified_over_default() {
        let lint = test_lint("unused_dependencies", &STYLE);

        let mut pkg_lints = TomlToolLints::new();
        pkg_lints.insert(
            "unused_dependencies".to_string(),
            cargo_util_schemas::manifest::TomlLint::Level(TomlLintLevel::Deny),
        );
        let features = Features::default();

        let (level, source) = lint.level(&pkg_lints, None, &features);
        assert_eq!(level, LintLevel::Deny);
        assert_eq!(source, LintLevelSource::Package);
    }

    #[test]
    fn lint_level_group_overrides_default() {
        let lint = test_lint("non_kebab_case_bins", &STYLE);

        let mut pkg_lints = TomlToolLints::new();
        pkg_lints.insert(
            "style".to_string(),
            cargo_util_schemas::manifest::TomlLint::Level(TomlLintLevel::Deny),
        );
        let features = Features::default();

        let (level, source) = lint.level(&pkg_lints, None, &features);
        assert_eq!(level, LintLevel::Deny);
        assert_eq!(source, LintLevelSource::Package);
    }
}
