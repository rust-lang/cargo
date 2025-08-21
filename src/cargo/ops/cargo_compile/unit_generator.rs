use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::core::Workspace;
use crate::core::compiler::UserIntent;
use crate::core::compiler::rustdoc::RustdocScrapeExamples;
use crate::core::compiler::unit_dependencies::IsArtifact;
use crate::core::compiler::{CompileKind, CompileMode, Unit};
use crate::core::compiler::{RustcTargetData, UnitInterner};
use crate::core::dependency::DepKind;
use crate::core::profiles::{Profiles, UnitFor};
use crate::core::resolver::features::{self, FeaturesFor};
use crate::core::resolver::{HasDevUnits, Resolve};
use crate::core::{FeatureValue, Package, PackageSet, Summary, Target};
use crate::util::restricted_names::is_glob_pattern;
use crate::util::{CargoResult, closest_msg};

use super::Packages;
use super::compile_filter::{CompileFilter, FilterRule, LibRule};
use super::packages::build_glob;

/// A proposed target.
///
/// Proposed targets are later filtered into actual `Unit`s based on whether or
/// not the target requires its features to be present.
#[derive(Debug)]
struct Proposal<'a> {
    pkg: &'a Package,
    target: &'a Target,
    /// Indicates whether or not all required features *must* be present. If
    /// false, and the features are not available, then it will be silently
    /// skipped. Generally, targets specified by name (`--bin foo`) are
    /// required, all others can be silently skipped if features are missing.
    requires_features: bool,
    mode: CompileMode,
}

/// The context needed for generating root units,
/// which are packages the user has requested to compile.
///
/// To generate a full [`UnitGraph`],
/// generally you need to call [`generate_root_units`] first,
/// and then provide the output to [`build_unit_dependencies`].
///
/// [`generate_root_units`]: UnitGenerator::generate_root_units
/// [`build_unit_dependencies`]: crate::core::compiler::unit_dependencies::build_unit_dependencies
/// [`UnitGraph`]: crate::core::compiler::unit_graph::UnitGraph
pub struct UnitGenerator<'a, 'gctx> {
    pub ws: &'a Workspace<'gctx>,
    pub packages: &'a [&'a Package],
    pub spec: &'a Packages,
    pub target_data: &'a RustcTargetData<'gctx>,
    pub filter: &'a CompileFilter,
    pub requested_kinds: &'a [CompileKind],
    pub explicit_host_kind: CompileKind,
    pub intent: UserIntent,
    pub resolve: &'a Resolve,
    pub workspace_resolve: &'a Option<Resolve>,
    pub resolved_features: &'a features::ResolvedFeatures,
    pub package_set: &'a PackageSet<'gctx>,
    pub profiles: &'a Profiles,
    pub interner: &'a UnitInterner,
    pub has_dev_units: HasDevUnits,
}

impl<'a> UnitGenerator<'a, '_> {
    /// Helper for creating a list of `Unit` structures
    fn new_units(
        &self,
        pkg: &Package,
        target: &Target,
        initial_target_mode: CompileMode,
    ) -> Vec<Unit> {
        // Custom build units are added in `build_unit_dependencies`.
        assert!(!target.is_custom_build());
        let target_mode = match initial_target_mode {
            CompileMode::Test => {
                if target.is_example() && !self.filter.is_specific() && !target.tested() {
                    // Examples are included as regular binaries to verify
                    // that they compile.
                    CompileMode::Build
                } else {
                    CompileMode::Test
                }
            }
            _ => initial_target_mode,
        };

        let is_local = pkg.package_id().source_id().is_path();

        // No need to worry about build-dependencies, roots are never build dependencies.
        let features_for = FeaturesFor::from_for_host(target.proc_macro());
        let features = self
            .resolved_features
            .activated_features(pkg.package_id(), features_for);

        // If `--target` has not been specified, then the unit
        // graph is built almost like if `--target $HOST` was
        // specified. See `rebuild_unit_graph_shared` for more on
        // why this is done. However, if the package has its own
        // `package.target` key, then this gets used instead of
        // `$HOST`
        let explicit_kinds = if let Some(k) = pkg.manifest().forced_kind() {
            vec![k]
        } else {
            self.requested_kinds
                .iter()
                .map(|kind| match kind {
                    CompileKind::Host => pkg
                        .manifest()
                        .default_kind()
                        .unwrap_or(self.explicit_host_kind),
                    CompileKind::Target(t) => CompileKind::Target(*t),
                })
                .collect()
        };

        explicit_kinds
            .into_iter()
            .map(move |kind| {
                let unit_for = if initial_target_mode.is_any_test() {
                    // NOTE: the `UnitFor` here is subtle. If you have a profile
                    // with `panic` set, the `panic` flag is cleared for
                    // tests/benchmarks and their dependencies. If this
                    // was `normal`, then the lib would get compiled three
                    // times (once with panic, once without, and once with
                    // `--test`).
                    //
                    // This would cause a problem for doc tests, which would fail
                    // because `rustdoc` would attempt to link with both libraries
                    // at the same time. Also, it's probably not important (or
                    // even desirable?) for rustdoc to link with a lib with
                    // `panic` set.
                    //
                    // As a consequence, Examples and Binaries get compiled
                    // without `panic` set. This probably isn't a bad deal.
                    //
                    // Forcing the lib to be compiled three times during `cargo
                    // test` is probably also not desirable.
                    UnitFor::new_test(self.ws.gctx(), kind)
                } else if target.for_host() {
                    // Proc macro / plugin should not have `panic` set.
                    UnitFor::new_compiler(kind)
                } else {
                    UnitFor::new_normal(kind)
                };
                let profile = self.profiles.get_profile(
                    pkg.package_id(),
                    self.ws.is_member(pkg),
                    is_local,
                    unit_for,
                    kind,
                );
                let kind = kind.for_target(target);
                self.interner.intern(
                    pkg,
                    target,
                    profile,
                    kind,
                    target_mode,
                    features.clone(),
                    self.target_data.info(kind).rustflags.clone(),
                    self.target_data.info(kind).rustdocflags.clone(),
                    self.target_data.target_config(kind).links_overrides.clone(),
                    /*is_std*/ false,
                    /*dep_hash*/ 0,
                    IsArtifact::No,
                    None,
                    false,
                )
            })
            .collect()
    }

    /// Given a list of all targets for a package, filters out only the targets
    /// that are automatically included when the user doesn't specify any targets.
    fn filter_default_targets<'b>(&self, targets: &'b [Target]) -> Vec<&'b Target> {
        match self.intent {
            UserIntent::Bench => targets.iter().filter(|t| t.benched()).collect(),
            UserIntent::Test => targets
                .iter()
                .filter(|t| t.tested() || t.is_example())
                .collect(),
            UserIntent::Build | UserIntent::Check { .. } => targets
                .iter()
                .filter(|t| t.is_bin() || t.is_lib())
                .collect(),
            UserIntent::Doc { .. } => {
                // `doc` does lib and bins (bin with same name as lib is skipped).
                targets
                    .iter()
                    .filter(|t| {
                        t.documented()
                            && (!t.is_bin()
                                || !targets
                                    .iter()
                                    .any(|l| l.is_lib() && l.crate_name() == t.crate_name()))
                    })
                    .collect()
            }
            UserIntent::Doctest => {
                panic!("Invalid intent {:?}", self.intent)
            }
        }
    }

    /// Filters the set of all possible targets based on the provided predicate.
    fn filter_targets(
        &self,
        predicate: impl Fn(&Target) -> bool,
        requires_features: bool,
        mode: CompileMode,
    ) -> Vec<Proposal<'a>> {
        self.packages
            .iter()
            .flat_map(|pkg| {
                pkg.targets()
                    .iter()
                    .filter(|t| predicate(t))
                    .map(|target| Proposal {
                        pkg,
                        target,
                        requires_features,
                        mode,
                    })
            })
            .collect()
    }

    /// Finds the targets for a specifically named target.
    fn find_named_targets(
        &self,
        target_name: &str,
        target_desc: &'static str,
        is_expected_kind: fn(&Target) -> bool,
        mode: CompileMode,
    ) -> CargoResult<Vec<Proposal<'a>>> {
        let is_glob = is_glob_pattern(target_name);
        let pattern = build_glob(target_name)?;
        let filter = |t: &Target| {
            if is_glob {
                is_expected_kind(t) && pattern.matches(t.name())
            } else {
                is_expected_kind(t) && t.name() == target_name
            }
        };
        let proposals = self.filter_targets(filter, true, mode);
        if proposals.is_empty() {
            let mut targets = std::collections::BTreeMap::new();
            for (pkg, target) in self.packages.iter().flat_map(|pkg| {
                pkg.targets()
                    .iter()
                    .filter(|target| is_expected_kind(target))
                    .map(move |t| (pkg, t))
            }) {
                targets
                    .entry(target.name())
                    .or_insert_with(Vec::new)
                    .push((pkg, target));
            }

            let suggestion = closest_msg(target_name, targets.keys(), |t| t, "target");
            let targets_elsewhere = self.get_targets_from_other_packages(filter)?;
            let append_targets_elsewhere = |msg: &mut String| {
                let mut available_msg = Vec::new();
                for (package, targets) in &targets_elsewhere {
                    if !targets.is_empty() {
                        available_msg.push(format!(
                            "help: available {target_desc} in `{package}` package:"
                        ));
                        for target in targets {
                            available_msg.push(format!("    {target}"));
                        }
                    }
                }
                if !available_msg.is_empty() {
                    write!(msg, "\n{}", available_msg.join("\n"))?;
                }
                CargoResult::Ok(())
            };

            let unmatched_packages = match self.spec {
                Packages::Default | Packages::OptOut(_) | Packages::All(_) => {
                    " in default-run packages".to_owned()
                }
                Packages::Packages(packages) => match packages.len() {
                    0 => String::new(),
                    1 => format!(" in `{}` package", packages[0]),
                    _ => format!(" in `{}`, ... packages", packages[0]),
                },
            };

            let named = if is_glob { "matches pattern" } else { "named" };

            let mut msg = String::new();
            write!(
                msg,
                "no {target_desc} target {named} `{target_name}`{unmatched_packages}{suggestion}",
            )?;
            if !targets_elsewhere.is_empty() {
                append_targets_elsewhere(&mut msg)?;
            } else if suggestion.is_empty() && !targets.is_empty() {
                write!(msg, "\nhelp: available {} targets:", target_desc)?;
                for (target_name, pkgs) in targets {
                    if pkgs.len() == 1 {
                        write!(msg, "\n    {target_name}")?;
                    } else {
                        for (pkg, _) in pkgs {
                            let pkg_name = pkg.name();
                            write!(msg, "\n    {target_name} in package {pkg_name}")?;
                        }
                    }
                }
            }
            anyhow::bail!(msg);
        }
        Ok(proposals)
    }

    fn get_targets_from_other_packages(
        &self,
        filter_fn: impl Fn(&Target) -> bool,
    ) -> CargoResult<Vec<(&str, Vec<&str>)>> {
        let packages = Packages::All(Vec::new()).get_packages(self.ws)?;
        let targets = packages
            .into_iter()
            .filter_map(|pkg| {
                let mut targets: Vec<_> = pkg
                    .manifest()
                    .targets()
                    .iter()
                    .filter_map(|target| filter_fn(target).then(|| target.name()))
                    .collect();
                if targets.is_empty() {
                    None
                } else {
                    targets.sort();
                    Some((pkg.name().as_str(), targets))
                }
            })
            .collect();

        Ok(targets)
    }

    /// Returns a list of proposed targets based on command-line target selection flags.
    fn list_rule_targets(
        &self,
        rule: &FilterRule,
        target_desc: &'static str,
        is_expected_kind: fn(&Target) -> bool,
        mode: CompileMode,
    ) -> CargoResult<Vec<Proposal<'a>>> {
        let mut proposals = Vec::new();
        match rule {
            FilterRule::All => proposals.extend(self.filter_targets(is_expected_kind, false, mode)),
            FilterRule::Just(names) => {
                for name in names {
                    proposals.extend(self.find_named_targets(
                        name,
                        target_desc,
                        is_expected_kind,
                        mode,
                    )?);
                }
            }
        }
        Ok(proposals)
    }

    /// Create a list of proposed targets given the context in `UnitGenerator`
    fn create_proposals(&self) -> CargoResult<Vec<Proposal<'_>>> {
        let mut proposals: Vec<Proposal<'_>> = Vec::new();

        match *self.filter {
            CompileFilter::Default {
                required_features_filterable,
            } => {
                for pkg in self.packages {
                    let default = self.filter_default_targets(pkg.targets());
                    proposals.extend(default.into_iter().map(|target| Proposal {
                        pkg,
                        target,
                        requires_features: !required_features_filterable,
                        mode: to_compile_mode(self.intent),
                    }));
                    if matches!(self.intent, UserIntent::Test) {
                        if let Some(t) = pkg
                            .targets()
                            .iter()
                            .find(|t| t.is_lib() && t.doctested() && t.doctestable())
                        {
                            proposals.push(Proposal {
                                pkg,
                                target: t,
                                requires_features: false,
                                mode: CompileMode::Doctest,
                            });
                        }
                    }
                }
            }
            CompileFilter::Only {
                all_targets,
                ref lib,
                ref bins,
                ref examples,
                ref tests,
                ref benches,
            } => {
                if *lib != LibRule::False {
                    let mut libs = Vec::new();
                    let compile_mode = to_compile_mode(self.intent);
                    for proposal in self.filter_targets(Target::is_lib, false, compile_mode) {
                        let Proposal { target, pkg, .. } = proposal;
                        if matches!(self.intent, UserIntent::Doctest) && !target.doctestable() {
                            let types = target.rustc_crate_types();
                            let types_str: Vec<&str> = types.iter().map(|t| t.as_str()).collect();
                            self.ws.gctx().shell().warn(format!(
                      "doc tests are not supported for crate type(s) `{}` in package `{}`",
                      types_str.join(", "),
                      pkg.name()
                  ))?;
                        } else {
                            libs.push(proposal)
                        }
                    }
                    if !all_targets && libs.is_empty() && *lib == LibRule::True {
                        let names = self
                            .packages
                            .iter()
                            .map(|pkg| pkg.name())
                            .collect::<Vec<_>>();
                        if names.len() == 1 {
                            anyhow::bail!("no library targets found in package `{}`", names[0]);
                        } else {
                            anyhow::bail!(
                                "no library targets found in packages: {}",
                                names.join(", ")
                            );
                        }
                    }
                    proposals.extend(libs);
                }

                let default_mode = to_compile_mode(self.intent);

                // If `--tests` was specified, add all targets that would be
                // generated by `cargo test`.
                let test_filter = match tests {
                    FilterRule::All => Target::tested,
                    FilterRule::Just(_) => Target::is_test,
                };
                let test_mode = match self.intent {
                    UserIntent::Build => CompileMode::Test,
                    UserIntent::Check { .. } => CompileMode::Check { test: true },
                    _ => default_mode,
                };
                // If `--benches` was specified, add all targets that would be
                // generated by `cargo bench`.
                let bench_filter = match benches {
                    FilterRule::All => Target::benched,
                    FilterRule::Just(_) => Target::is_bench,
                };

                proposals.extend(self.list_rule_targets(
                    bins,
                    "bin",
                    Target::is_bin,
                    default_mode,
                )?);
                proposals.extend(self.list_rule_targets(
                    examples,
                    "example",
                    Target::is_example,
                    default_mode,
                )?);
                proposals.extend(self.list_rule_targets(tests, "test", test_filter, test_mode)?);
                proposals.extend(self.list_rule_targets(
                    benches,
                    "bench",
                    bench_filter,
                    test_mode,
                )?);
            }
        }

        Ok(proposals)
    }

    /// Proposes targets from which to scrape examples for documentation
    fn create_docscrape_proposals(&self, doc_units: &[Unit]) -> CargoResult<Vec<Proposal<'a>>> {
        // In general, the goal is to scrape examples from (a) whatever targets
        // the user is documenting, and (b) Example targets. However, if the user
        // is documenting a library with dev-dependencies, those dev-deps are not
        // needed for the library, while dev-deps are needed for the examples.
        //
        // If scrape-examples caused `cargo doc` to start requiring dev-deps, this
        // would be a breaking change to crates whose dev-deps don't compile.
        // Therefore we ONLY want to scrape Example targets if either:
        //    (1) No package has dev-dependencies, so this is a moot issue, OR
        //    (2) The provided CompileFilter requires dev-dependencies anyway.
        //
        // The next two variables represent these two conditions.
        let no_pkg_has_dev_deps = self.packages.iter().all(|pkg| {
            pkg.summary()
                .dependencies()
                .iter()
                .all(|dep| !matches!(dep.kind(), DepKind::Development))
        });
        let reqs_dev_deps = matches!(self.has_dev_units, HasDevUnits::Yes);
        let safe_to_scrape_example_targets = no_pkg_has_dev_deps || reqs_dev_deps;

        let pkgs_to_scrape = doc_units
            .iter()
            .filter(|unit| self.ws.unit_needs_doc_scrape(unit))
            .map(|u| &u.pkg)
            .collect::<HashSet<_>>();

        let skipped_examples = RefCell::new(Vec::new());
        let can_scrape = |target: &Target| {
            match (target.doc_scrape_examples(), target.is_example()) {
                // Targets configured by the user to not be scraped should never be scraped
                (RustdocScrapeExamples::Disabled, _) => false,
                // Targets configured by the user to be scraped should always be scraped
                (RustdocScrapeExamples::Enabled, _) => true,
                // Example targets with no configuration should be conditionally scraped if
                // it's guaranteed not to break the build
                (RustdocScrapeExamples::Unset, true) => {
                    if !safe_to_scrape_example_targets {
                        skipped_examples
                            .borrow_mut()
                            .push(target.name().to_string());
                    }
                    safe_to_scrape_example_targets
                }
                // All other targets are ignored for now. This may change in the future!
                (RustdocScrapeExamples::Unset, false) => false,
            }
        };

        let mut scrape_proposals = self.filter_targets(can_scrape, false, CompileMode::Docscrape);
        scrape_proposals.retain(|proposal| pkgs_to_scrape.contains(proposal.pkg));

        let skipped_examples = skipped_examples.into_inner();
        if !skipped_examples.is_empty() {
            let mut shell = self.ws.gctx().shell();
            let example_str = skipped_examples.join(", ");
            shell.warn(format!(
                "\
Rustdoc did not scrape the following examples because they require dev-dependencies: {example_str}
    If you want Rustdoc to scrape these examples, then add `doc-scrape-examples = true`
    to the [[example]] target configuration of at least one example."
            ))?;
        }

        Ok(scrape_proposals)
    }

    /// Checks if the unit list is empty and the user has passed any combination of
    /// --tests, --examples, --benches or --bins, and we didn't match on any targets.
    /// We want to emit a warning to make sure the user knows that this run is a no-op,
    /// and their code remains unchecked despite cargo not returning any errors
    fn unmatched_target_filters(&self, units: &[Unit]) -> CargoResult<()> {
        let mut shell = self.ws.gctx().shell();
        if let CompileFilter::Only {
            all_targets,
            lib: _,
            ref bins,
            ref examples,
            ref tests,
            ref benches,
        } = *self.filter
        {
            if units.is_empty() {
                let mut filters = String::new();
                let mut miss_count = 0;

                let mut append = |t: &FilterRule, s| {
                    if let FilterRule::All = *t {
                        miss_count += 1;
                        filters.push_str(s);
                    }
                };

                if all_targets {
                    filters.push_str(" `all-targets`");
                } else {
                    append(bins, " `bins`,");
                    append(tests, " `tests`,");
                    append(examples, " `examples`,");
                    append(benches, " `benches`,");
                    filters.pop();
                }

                return shell.warn(format!(
                    "target {}{} specified, but no targets matched; this is a no-op",
                    if miss_count > 1 { "filters" } else { "filter" },
                    filters,
                ));
            }
        }

        Ok(())
    }

    /// Warns if a target's required-features references a feature that doesn't exist.
    ///
    /// This is a warning because historically this was not validated, and it
    /// would cause too much breakage to make it an error.
    fn validate_required_features(
        &self,
        target_name: &str,
        required_features: &[String],
        summary: &Summary,
    ) -> CargoResult<()> {
        let resolve = match self.workspace_resolve {
            None => return Ok(()),
            Some(resolve) => resolve,
        };

        let mut shell = self.ws.gctx().shell();
        for feature in required_features {
            let fv = FeatureValue::new(feature.into());
            match &fv {
                FeatureValue::Feature(f) => {
                    if !summary.features().contains_key(f) {
                        shell.warn(format!(
                            "invalid feature `{}` in required-features of target `{}`: \
                      `{}` is not present in [features] section",
                            fv, target_name, fv
                        ))?;
                    }
                }
                FeatureValue::Dep { .. } => {
                    anyhow::bail!(
                        "invalid feature `{}` in required-features of target `{}`: \
                  `dep:` prefixed feature values are not allowed in required-features",
                        fv,
                        target_name
                    );
                }
                FeatureValue::DepFeature { weak: true, .. } => {
                    anyhow::bail!(
                        "invalid feature `{}` in required-features of target `{}`: \
                  optional dependency with `?` is not allowed in required-features",
                        fv,
                        target_name
                    );
                }
                // Handling of dependent_crate/dependent_crate_feature syntax
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature,
                    weak: false,
                } => {
                    match resolve.deps(summary.package_id()).find(|(_dep_id, deps)| {
                        deps.iter().any(|dep| dep.name_in_toml() == *dep_name)
                    }) {
                        Some((dep_id, _deps)) => {
                            let dep_summary = resolve.summary(dep_id);
                            if !dep_summary.features().contains_key(dep_feature)
                                && !dep_summary.dependencies().iter().any(|dep| {
                                    dep.name_in_toml() == *dep_feature && dep.is_optional()
                                })
                            {
                                shell.warn(format!(
                                    "invalid feature `{}` in required-features of target `{}`: \
                              feature `{}` does not exist in package `{}`",
                                    fv, target_name, dep_feature, dep_id
                                ))?;
                            }
                        }
                        None => {
                            shell.warn(format!(
                                "invalid feature `{}` in required-features of target `{}`: \
                          dependency `{}` does not exist",
                                fv, target_name, dep_name
                            ))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Converts proposals to units based on each target's required features.
    fn proposals_to_units(&self, proposals: Vec<Proposal<'_>>) -> CargoResult<Vec<Unit>> {
        // Only include targets that are libraries or have all required
        // features available.
        //
        // `features_map` is a map of &Package -> enabled_features
        // It is computed by the set of enabled features for the package plus
        // every enabled feature of every enabled dependency.
        let mut features_map = HashMap::new();
        // This needs to be a set to de-duplicate units. Due to the way the
        // targets are filtered, it is possible to have duplicate proposals for
        // the same thing.
        let mut units = HashSet::new();
        for Proposal {
            pkg,
            target,
            requires_features,
            mode,
        } in proposals
        {
            let unavailable_features = match target.required_features() {
                Some(rf) => {
                    self.validate_required_features(target.name(), rf, pkg.summary())?;

                    let features = features_map.entry(pkg).or_insert_with(|| {
                        super::resolve_all_features(
                            self.resolve,
                            self.resolved_features,
                            self.package_set,
                            pkg.package_id(),
                        )
                    });
                    rf.iter().filter(|f| !features.contains(*f)).collect()
                }
                None => Vec::new(),
            };
            if target.is_lib() || unavailable_features.is_empty() {
                units.extend(self.new_units(pkg, target, mode));
            } else if requires_features {
                let required_features = target.required_features().unwrap();
                let quoted_required_features: Vec<String> = required_features
                    .iter()
                    .map(|s| format!("`{}`", s))
                    .collect();
                anyhow::bail!(
                    "target `{}` in package `{}` requires the features: {}\n\
               Consider enabling them by passing, e.g., `--features=\"{}\"`",
                    target.name(),
                    pkg.name(),
                    quoted_required_features.join(", "),
                    required_features.join(" ")
                );
            }
            // else, silently skip target.
        }
        let mut units: Vec<_> = units.into_iter().collect();
        self.unmatched_target_filters(&units)?;

        // Keep the roots in a consistent order, which helps with checking test output.
        units.sort_unstable();
        Ok(units)
    }

    /// Generates all the base units for the packages the user has requested to
    /// compile. Dependencies for these units are computed later in [`unit_dependencies`].
    ///
    /// [`unit_dependencies`]: crate::core::compiler::unit_dependencies
    pub fn generate_root_units(&self) -> CargoResult<Vec<Unit>> {
        let proposals = self.create_proposals()?;
        self.proposals_to_units(proposals)
    }

    /// Generates units specifically for doc-scraping.
    ///
    /// This requires a separate entrypoint from [`generate_root_units`] because it
    /// takes the documented units as input.
    ///
    /// [`generate_root_units`]: Self::generate_root_units
    pub fn generate_scrape_units(&self, doc_units: &[Unit]) -> CargoResult<Vec<Unit>> {
        let scrape_proposals = self.create_docscrape_proposals(&doc_units)?;
        let scrape_units = self.proposals_to_units(scrape_proposals)?;
        Ok(scrape_units)
    }
}

/// Converts [`UserIntent`] to [`CompileMode`] for root units.
fn to_compile_mode(intent: UserIntent) -> CompileMode {
    match intent {
        UserIntent::Test | UserIntent::Bench => CompileMode::Test,
        UserIntent::Build => CompileMode::Build,
        UserIntent::Check { test } => CompileMode::Check { test },
        UserIntent::Doc { .. } => CompileMode::Doc,
        UserIntent::Doctest => CompileMode::Doctest,
    }
}
