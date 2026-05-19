mod blanket_hint_mostly_unused;
mod deferred_parse_diagnostics;
mod im_a_teapot;
mod implicit_minimum_version_req;
mod missing_lints_features;
mod missing_lints_inheritance;
mod non_kebab_case_bins;
mod non_kebab_case_features;
mod non_kebab_case_packages;
mod non_snake_case_features;
mod non_snake_case_packages;
mod redundant_homepage;
mod redundant_readme;
mod text_direction_codepoint_in_comment;
mod text_direction_codepoint_in_literal;
mod unknown_lints;
pub mod unused_dependencies;
mod unused_workspace_dependencies;
mod unused_workspace_package_fields;

use super::LintGroup;
use super::LintLevel;
use super::passes::ParsePassRule;
use crate::core::Feature;

pub const PARSE_PASS_RULES: &[ParsePassRule<'static>] = &[
    ParsePassRule::DiagnosticManifest {
        rule: deferred_parse_diagnostics::diagnose_manifest,
    },
    ParsePassRule::DiagnosticManifest {
        rule: missing_lints_features::diagnose_manifest,
    },
    ParsePassRule::LintManifest {
        rule: unknown_lints::lint_manifest,
        lint: unknown_lints::LINT,
    },
    ParsePassRule::LintWorkspace {
        rule: unused_workspace_package_fields::lint_workspace,
        lint: unused_workspace_package_fields::LINT,
    },
    ParsePassRule::LintWorkspace {
        rule: unused_workspace_dependencies::lint_workspace,
        lint: unused_workspace_dependencies::LINT,
    },
    ParsePassRule::LintWorkspace {
        rule: implicit_minimum_version_req::lint_workspace,
        lint: implicit_minimum_version_req::LINT,
    },
    ParsePassRule::LintManifest {
        rule: text_direction_codepoint_in_comment::lint_manifest,
        lint: text_direction_codepoint_in_comment::LINT,
    },
    ParsePassRule::LintManifest {
        rule: text_direction_codepoint_in_literal::lint_manifest,
        lint: text_direction_codepoint_in_literal::LINT,
    },
    ParsePassRule::LintWorkspace {
        rule: blanket_hint_mostly_unused::lint_workspace,
        lint: blanket_hint_mostly_unused::LINT,
    },
    ParsePassRule::LintPackage {
        rule: im_a_teapot::lint_package,
        lint: im_a_teapot::LINT,
    },
    ParsePassRule::LintPackage {
        rule: implicit_minimum_version_req::lint_package,
        lint: implicit_minimum_version_req::LINT,
    },
    ParsePassRule::LintPackage {
        rule: non_kebab_case_packages::lint_package,
        lint: non_kebab_case_packages::LINT,
    },
    ParsePassRule::LintPackage {
        rule: non_snake_case_packages::lint_package,
        lint: non_snake_case_packages::LINT,
    },
    ParsePassRule::LintPackage {
        rule: non_kebab_case_bins::lint_package,
        lint: non_kebab_case_bins::LINT,
    },
    ParsePassRule::LintPackage {
        rule: non_kebab_case_features::lint_package,
        lint: non_kebab_case_features::LINT,
    },
    ParsePassRule::LintPackage {
        rule: non_snake_case_features::lint_package,
        lint: non_snake_case_features::LINT,
    },
    ParsePassRule::LintPackage {
        rule: unused_dependencies::lint_package,
        lint: unused_dependencies::LINT,
    },
    ParsePassRule::LintPackage {
        rule: redundant_readme::lint_package,
        lint: redundant_readme::LINT,
    },
    ParsePassRule::LintPackage {
        rule: redundant_homepage::lint_package,
        lint: redundant_homepage::LINT,
    },
    ParsePassRule::LintPackage {
        rule: missing_lints_inheritance::lint_package,
        lint: missing_lints_inheritance::LINT,
    },
];

pub static LINTS: &[&crate::diagnostics::Lint] = &[
    blanket_hint_mostly_unused::LINT,
    implicit_minimum_version_req::LINT,
    im_a_teapot::LINT,
    missing_lints_inheritance::LINT,
    non_kebab_case_bins::LINT,
    non_kebab_case_features::LINT,
    non_kebab_case_packages::LINT,
    non_snake_case_features::LINT,
    non_snake_case_packages::LINT,
    redundant_homepage::LINT,
    redundant_readme::LINT,
    text_direction_codepoint_in_comment::LINT,
    text_direction_codepoint_in_literal::LINT,
    unknown_lints::LINT,
    unused_dependencies::LINT,
    unused_workspace_dependencies::LINT,
    unused_workspace_package_fields::LINT,
];

/// Version required for specifying `[lints.cargo]`
///
/// Before this, it was an error.  No on-by-default lint should fire before this time without
/// another way of disabling it.
static CARGO_LINTS_MSRV: cargo_util_schemas::manifest::RustVersion =
    cargo_util_schemas::manifest::RustVersion::new(1, 79, 0);

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
    feature_gate: Some(crate::core::Feature::test_dummy_unstable()),
    hidden: true,
};

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

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use snapbox::ToDebug;
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ensure_lint_groups_do_not_default_to_forbid() {
        let forbid_groups = LINT_GROUPS
            .iter()
            .filter(|g| matches!(g.default_level, LintLevel::Forbid))
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

        let actual = LINTS
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
        let actual = LINT_GROUPS
            .iter()
            .map(|l| l.name.to_uppercase())
            .collect::<Vec<_>>();

        let mut expected = actual.clone();
        expected.sort();
        snapbox::assert_data_eq!(actual.to_debug(), expected.to_debug());
    }

    #[test]
    fn ensure_parse_passed_in_lints() {
        let parse_pass_lint_names = PARSE_PASS_RULES
            .iter()
            .filter_map(|rule| match rule {
                ParsePassRule::DiagnosticManifest { .. }
                | ParsePassRule::DiagnosticWorkspace { .. }
                | ParsePassRule::DiagnosticPackage { .. } => None,
                ParsePassRule::LintManifest { lint, .. }
                | ParsePassRule::LintWorkspace { lint, .. }
                | ParsePassRule::LintPackage { lint, .. } => Some(lint.name),
            })
            .collect::<std::collections::HashSet<_>>();
        let lint_names = LINTS
            .iter()
            .map(|l| l.name)
            .collect::<std::collections::HashSet<_>>();
        let diff = parse_pass_lint_names
            .difference(&lint_names)
            .sorted()
            .collect::<Vec<_>>();
        let mut need_added = String::new();
        for name in &diff {
            need_added.push_str(&format!("{name}\n"));
        }
        assert!(
            diff.is_empty(),
            "\n`LINTS` did not contain all `Lint`s found in `PARSE_PASS_RULES`\n\
            Please add the following to `LINTS`:\n\
            {need_added}",
        );
    }

    #[test]
    fn ensure_updated_lints() {
        let dir = snapbox::utils::current_dir!();
        let mut expected = HashSet::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.ends_with("mod.rs") {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap();
            if !content.contains("LINT") {
                // diagnostic
                continue;
            }
            let lint_name = path.file_stem().unwrap().to_string_lossy();
            assert!(expected.insert(lint_name.into()), "duplicate lint found");
        }

        let actual = LINTS
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
        let actual = LINT_GROUPS
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
