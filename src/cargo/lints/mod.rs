use std::path::Path;

use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Snippet;

use crate::core::Workspace;
use crate::core::{Edition, Feature, Features, MaybePackage, Package};
use crate::{CargoResult, GlobalContext};

mod lint;
mod report;

pub mod rules;

pub use lint::{Lint, LintGroup, LintLevel, LintLevelSource};
pub use report::{AsIndex, get_key_value, get_key_value_span, rel_cwd_manifest_path};
pub use rules::LINTS;

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
    fn lint_level(&self, pkg_lints: &TomlToolLints, lint: &Lint) -> (LintLevel, LintLevelSource) {
        lint.level(pkg_lints, self.rust_version(), self.unstable_features())
    }

    pub fn rust_version(&self) -> Option<&RustVersion> {
        match self {
            ManifestFor::Package(p) => p.rust_version(),
            ManifestFor::Workspace { ws, maybe_pkg: _ } => ws.lowest_rust_version(),
        }
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
        let Some((name, default_level, feature_gate)) = find_lint_or_group(lint_name) else {
            unknown_lints.push(lint_name);
            continue;
        };

        let (_, source, _) = lint::level_priority(name, *default_level, cargo_lints);

        // Only run analysis on user-specified lints
        if !source.is_user_specified() {
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
) -> Option<(&'static str, &LintLevel, &Option<&'static Feature>)> {
    if let Some(lint) = LINTS.iter().find(|l| l.name == name) {
        Some((
            lint.name,
            &lint.primary_group.default_level,
            &lint.feature_gate,
        ))
    } else if let Some(group) = LINT_GROUPS.iter().find(|g| g.name == name) {
        Some((group.name, &group.default_level, &group.feature_gate))
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

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use snapbox::ToDebug;
    use std::collections::HashSet;

    #[test]
    fn ensure_lint_groups_do_not_default_to_forbid() {
        let forbid_groups = super::LINT_GROUPS
            .iter()
            .filter(|g| matches!(g.default_level, super::LintLevel::Forbid))
            .collect::<Vec<_>>();

        assert!(
            forbid_groups.is_empty(),
            "\n`LintGroup`s should never default to `forbid`, but the following do:\n\
            {}\n",
            forbid_groups.iter().map(|g| g.name).join("\n")
        );
    }

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
