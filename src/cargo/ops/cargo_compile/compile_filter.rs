//! Filters and their rules to select which Cargo targets will be built.

use crate::core::compiler::CompileMode;

use crate::core::{Target, TargetKind};
use crate::util::restricted_names::is_glob_pattern;

#[derive(Debug, PartialEq, Eq, Clone)]
/// Indicates whether or not the library target gets included.
pub enum LibRule {
    /// Include the library, fail if not present
    True,
    /// Include the library if present
    Default,
    /// Exclude the library
    False,
}

#[derive(Debug, Clone)]
/// Indicates which Cargo targets will be selected to be built.
pub enum FilterRule {
    /// All included.
    All,
    /// Just a subset of Cargo targets based on names given.
    Just(Vec<String>),
}

/// Filter to apply to the root package to select which Cargo targets will be built.
/// (examples, bins, benches, tests, ...)
///
/// The actual filter process happens inside [`generate_root_units`].
///
/// Not to be confused with [`Packages`], which opts in packages to be built.
///
/// [`generate_root_units`]: super::UnitGenerator::generate_root_units
/// [`Packages`]: crate::ops::Packages
#[derive(Debug, Clone)]
pub enum CompileFilter {
    /// The default set of Cargo targets.
    Default {
        /// Flag whether targets can be safely skipped when required-features are not satisfied.
        required_features_filterable: bool,
    },
    /// Only includes a subset of all Cargo targets.
    Only {
        /// Include all Cargo targets.
        all_targets: bool,
        lib: LibRule,
        bins: FilterRule,
        examples: FilterRule,
        tests: FilterRule,
        benches: FilterRule,
    },
}

impl FilterRule {
    pub fn new(targets: Vec<String>, all: bool) -> FilterRule {
        if all {
            FilterRule::All
        } else {
            FilterRule::Just(targets)
        }
    }

    /// Creates a filter with no rule.
    ///
    /// In the current Cargo implementation, filter without a rule implies
    /// Cargo will follows the default behaviour to filter targets.
    pub fn none() -> FilterRule {
        FilterRule::Just(Vec::new())
    }

    /// Checks if a target definition matches this filter rule.
    fn matches(&self, target: &Target) -> bool {
        match *self {
            FilterRule::All => true,
            FilterRule::Just(ref targets) => targets.iter().any(|x| *x == target.name()),
        }
    }

    /// Check if a filter is specific.
    ///
    /// Only filters without rules are considered as not specific.
    fn is_specific(&self) -> bool {
        match *self {
            FilterRule::All => true,
            FilterRule::Just(ref targets) => !targets.is_empty(),
        }
    }

    /// Checks if any specified target name contains glob patterns.
    pub(crate) fn contains_glob_patterns(&self) -> bool {
        match self {
            FilterRule::All => false,
            FilterRule::Just(targets) => targets.iter().any(is_glob_pattern),
        }
    }
}

impl CompileFilter {
    /// Constructs a filter from raw command line arguments.
    pub fn from_raw_arguments(
        lib_only: bool,
        bins: Vec<String>,
        all_bins: bool,
        tsts: Vec<String>,
        all_tsts: bool,
        exms: Vec<String>,
        all_exms: bool,
        bens: Vec<String>,
        all_bens: bool,
        all_targets: bool,
    ) -> CompileFilter {
        if all_targets {
            return CompileFilter::new_all_targets();
        }
        let rule_lib = if lib_only {
            LibRule::True
        } else {
            LibRule::False
        };
        let rule_bins = FilterRule::new(bins, all_bins);
        let rule_tsts = FilterRule::new(tsts, all_tsts);
        let rule_exms = FilterRule::new(exms, all_exms);
        let rule_bens = FilterRule::new(bens, all_bens);

        CompileFilter::new(rule_lib, rule_bins, rule_tsts, rule_exms, rule_bens)
    }

    /// Constructs a filter from underlying primitives.
    pub fn new(
        rule_lib: LibRule,
        rule_bins: FilterRule,
        rule_tsts: FilterRule,
        rule_exms: FilterRule,
        rule_bens: FilterRule,
    ) -> CompileFilter {
        if rule_lib == LibRule::True
            || rule_bins.is_specific()
            || rule_tsts.is_specific()
            || rule_exms.is_specific()
            || rule_bens.is_specific()
        {
            CompileFilter::Only {
                all_targets: false,
                lib: rule_lib,
                bins: rule_bins,
                examples: rule_exms,
                benches: rule_bens,
                tests: rule_tsts,
            }
        } else {
            CompileFilter::Default {
                required_features_filterable: true,
            }
        }
    }

    /// Constructs a filter that includes all targets.
    pub fn new_all_targets() -> CompileFilter {
        CompileFilter::Only {
            all_targets: true,
            lib: LibRule::Default,
            bins: FilterRule::All,
            examples: FilterRule::All,
            benches: FilterRule::All,
            tests: FilterRule::All,
        }
    }

    /// Constructs a filter that includes all test targets.
    ///
    /// Being different from the behavior of [`CompileFilter::Default`], this
    /// function only recognizes test targets, which means cargo might compile
    /// all targets with `tested` flag on, whereas [`CompileFilter::Default`]
    /// may include additional example targets to ensure they can be compiled.
    ///
    /// Note that the actual behavior is subject to [`filter_default_targets`]
    /// and [`generate_root_units`] though.
    ///
    /// [`generate_root_units`]: super::UnitGenerator::generate_root_units
    /// [`filter_default_targets`]: super::UnitGenerator::filter_default_targets
    pub fn all_test_targets() -> Self {
        Self::Only {
            all_targets: false,
            lib: LibRule::Default,
            bins: FilterRule::none(),
            examples: FilterRule::none(),
            tests: FilterRule::All,
            benches: FilterRule::none(),
        }
    }

    /// Constructs a filter that includes lib target only.
    pub fn lib_only() -> Self {
        Self::Only {
            all_targets: false,
            lib: LibRule::True,
            bins: FilterRule::none(),
            examples: FilterRule::none(),
            tests: FilterRule::none(),
            benches: FilterRule::none(),
        }
    }

    /// Constructs a filter that includes the given binary. No more. No less.
    pub fn single_bin(bin: String) -> Self {
        Self::Only {
            all_targets: false,
            lib: LibRule::False,
            bins: FilterRule::new(vec![bin], false),
            examples: FilterRule::none(),
            tests: FilterRule::none(),
            benches: FilterRule::none(),
        }
    }

    /// Indicates if Cargo needs to build any dev dependency.
    pub fn need_dev_deps(&self, mode: CompileMode) -> bool {
        match mode {
            CompileMode::Test | CompileMode::Doctest | CompileMode::Bench => true,
            CompileMode::Check { test: true } => true,
            CompileMode::Build
            | CompileMode::Doc { .. }
            | CompileMode::Docscrape
            | CompileMode::Check { test: false } => match *self {
                CompileFilter::Default { .. } => false,
                CompileFilter::Only {
                    ref examples,
                    ref tests,
                    ref benches,
                    ..
                } => examples.is_specific() || tests.is_specific() || benches.is_specific(),
            },
            CompileMode::RunCustomBuild => panic!("Invalid mode"),
        }
    }

    /// Selects targets for "cargo run". for logic to select targets for other
    /// subcommands, see [`generate_root_units`] and [`filter_default_targets`].
    ///
    /// [`generate_root_units`]: super::UnitGenerator::generate_root_units
    /// [`filter_default_targets`]: super::UnitGenerator::filter_default_targets
    pub fn target_run(&self, target: &Target) -> bool {
        match *self {
            CompileFilter::Default { .. } => true,
            CompileFilter::Only {
                ref lib,
                ref bins,
                ref examples,
                ref tests,
                ref benches,
                ..
            } => {
                let rule = match *target.kind() {
                    TargetKind::Bin => bins,
                    TargetKind::Test => tests,
                    TargetKind::Bench => benches,
                    TargetKind::ExampleBin | TargetKind::ExampleLib(..) => examples,
                    TargetKind::Lib(..) => {
                        return match *lib {
                            LibRule::True => true,
                            LibRule::Default => true,
                            LibRule::False => false,
                        };
                    }
                    TargetKind::CustomBuild => return false,
                };
                rule.matches(target)
            }
        }
    }

    pub fn is_specific(&self) -> bool {
        match *self {
            CompileFilter::Default { .. } => false,
            CompileFilter::Only { .. } => true,
        }
    }

    pub fn is_all_targets(&self) -> bool {
        matches!(
            *self,
            CompileFilter::Only {
                all_targets: true,
                ..
            }
        )
    }

    /// Checks if any specified target name contains glob patterns.
    pub(crate) fn contains_glob_patterns(&self) -> bool {
        match self {
            CompileFilter::Default { .. } => false,
            CompileFilter::Only {
                bins,
                examples,
                tests,
                benches,
                ..
            } => {
                bins.contains_glob_patterns()
                    || examples.contains_glob_patterns()
                    || tests.contains_glob_patterns()
                    || benches.contains_glob_patterns()
            }
        }
    }
}
