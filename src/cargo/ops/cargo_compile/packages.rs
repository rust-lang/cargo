//! See [`Packages`].

use std::collections::BTreeSet;

use crate::core::{Package, PackageIdSpecQuery};
use crate::core::{PackageIdSpec, Workspace};
use crate::util::CargoResult;
use crate::util::restricted_names::is_glob_pattern;

use anyhow::{Context as _, bail};

/// Represents the selected packages that will be built.
///
/// Generally, it represents the combination of all `-p` flag. When working within
/// a workspace, `--exclude` and `--workspace` flags also contribute to it.
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Packages {
    /// Packages selected by default. Usually means no flag provided.
    Default,
    /// Opt in all packages.
    ///
    /// As of the time of this writing, it only works on opting in all workspace members.
    /// Keeps the packages passed in to verify that they exist in the workspace.
    All(Vec<String>),
    /// Opt out of packages passed in.
    ///
    /// As of the time of this writing, it only works on opting out workspace members.
    OptOut(Vec<String>),
    /// A sequence of hand-picked packages that will be built. Normally done by `-p` flag.
    Packages(Vec<String>),
}

impl Packages {
    /// Creates a `Packages` from flags which are generally equivalent to command line flags.
    pub fn from_flags(all: bool, exclude: Vec<String>, package: Vec<String>) -> CargoResult<Self> {
        Ok(match (all, exclude.len(), package.len()) {
            (false, 0, 0) => Packages::Default,
            (false, 0, _) => Packages::Packages(package),
            (false, _, _) => anyhow::bail!("--exclude can only be used together with --workspace"),
            (true, 0, _) => Packages::All(package),
            (true, _, _) => Packages::OptOut(exclude),
        })
    }

    /// Converts selected packages to [`PackageIdSpec`]s.
    pub fn to_package_id_specs(&self, ws: &Workspace<'_>) -> CargoResult<Vec<PackageIdSpec>> {
        let specs = match self {
            Packages::All(packages) => {
                emit_packages_not_found_within_workspace(ws, packages)?;
                ws.members()
                    .map(Package::package_id)
                    .map(|id| id.to_spec())
                    .collect()
            }
            Packages::OptOut(opt_out) => {
                let (mut patterns, mut ids) = opt_patterns_and_ids(opt_out)?;
                let specs = ws
                    .members()
                    .filter(|pkg| {
                        let id = ids.iter().find(|id| id.matches(pkg.package_id())).cloned();
                        if let Some(id) = &id {
                            ids.remove(id);
                        }
                        !id.is_some() && !match_patterns(pkg, &mut patterns)
                    })
                    .map(Package::package_id)
                    .map(|id| id.to_spec())
                    .collect();
                let warn = |e| ws.gctx().shell().warn(e);
                let names = ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<BTreeSet<_>>();
                emit_package_not_found(ws, names, true).or_else(warn)?;
                emit_pattern_not_found(ws, patterns, true).or_else(warn)?;
                specs
            }
            Packages::Packages(packages) if packages.is_empty() => {
                vec![ws.current()?.package_id().to_spec()]
            }
            Packages::Packages(opt_in) => {
                let (mut patterns, mut specs) = opt_patterns_and_ids(opt_in)?;
                if !patterns.is_empty() {
                    let matched_pkgs = ws
                        .members()
                        .filter(|pkg| match_patterns(pkg, &mut patterns))
                        .map(Package::package_id)
                        .map(|id| id.to_spec());
                    specs.extend(matched_pkgs);
                }
                emit_pattern_not_found(ws, patterns, false)?;
                specs.into_iter().collect()
            }
            Packages::Default => ws
                .default_members()
                .map(Package::package_id)
                .map(|id| id.to_spec())
                .collect(),
        };
        if specs.is_empty() {
            if ws.is_virtual() {
                bail!(
                    "manifest path `{}` contains no package: The manifest is virtual, \
                     and the workspace has no members.",
                    ws.root().display()
                )
            }
            bail!("no packages to compile")
        }
        Ok(specs)
    }

    /// Gets a list of selected [`Package`]s.
    pub fn get_packages<'ws>(&self, ws: &'ws Workspace<'_>) -> CargoResult<Vec<&'ws Package>> {
        let packages: Vec<_> = match self {
            Packages::Default => ws.default_members().collect(),
            Packages::All(packages) => {
                emit_packages_not_found_within_workspace(ws, packages)?;
                ws.members().collect()
            }
            Packages::OptOut(opt_out) => {
                let (mut patterns, mut ids) = opt_patterns_and_ids(opt_out)?;
                let packages = ws
                    .members()
                    .filter(|pkg| {
                        let id = ids.iter().find(|id| id.matches(pkg.package_id())).cloned();
                        if let Some(id) = &id {
                            ids.remove(id);
                        }
                        !id.is_some() && !match_patterns(pkg, &mut patterns)
                    })
                    .collect();
                let names = ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<BTreeSet<_>>();
                emit_package_not_found(ws, names, true)?;
                emit_pattern_not_found(ws, patterns, true)?;
                packages
            }
            Packages::Packages(opt_in) => {
                let (mut patterns, mut ids) = opt_patterns_and_ids(opt_in)?;
                let packages = ws
                    .members()
                    .filter(|pkg| {
                        let id = ids.iter().find(|id| id.matches(pkg.package_id())).cloned();
                        if let Some(id) = &id {
                            ids.remove(id);
                        }
                        id.is_some() || match_patterns(pkg, &mut patterns)
                    })
                    .collect();
                let names = ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<BTreeSet<_>>();
                emit_package_not_found(ws, names, false)?;
                emit_pattern_not_found(ws, patterns, false)?;
                packages
            }
        };
        Ok(packages)
    }

    /// Returns whether or not the user needs to pass a `-p` flag to target a
    /// specific package in the workspace.
    pub fn needs_spec_flag(&self, ws: &Workspace<'_>) -> bool {
        match self {
            Packages::Default => ws.default_members().count() > 1,
            Packages::All(_) => ws.members().count() > 1,
            Packages::Packages(_) => true,
            Packages::OptOut(_) => true,
        }
    }
}

/// Emits "package not found" error.
fn emit_package_not_found(
    ws: &Workspace<'_>,
    opt_names: BTreeSet<String>,
    opt_out: bool,
) -> CargoResult<()> {
    if !opt_names.is_empty() {
        anyhow::bail!(
            "{}package(s) `{}` not found in workspace `{}`",
            if opt_out { "excluded " } else { "" },
            opt_names.into_iter().collect::<Vec<_>>().join(", "),
            ws.root().display(),
        )
    }
    Ok(())
}

/// Emits "glob pattern not found" error.
fn emit_pattern_not_found(
    ws: &Workspace<'_>,
    opt_patterns: Vec<(glob::Pattern, bool)>,
    opt_out: bool,
) -> CargoResult<()> {
    let not_matched = opt_patterns
        .iter()
        .filter(|(_, matched)| !*matched)
        .map(|(pat, _)| pat.as_str())
        .collect::<Vec<_>>();
    if !not_matched.is_empty() {
        anyhow::bail!(
            "{}package pattern(s) `{}` not found in workspace `{}`",
            if opt_out { "excluded " } else { "" },
            not_matched.join(", "),
            ws.root().display(),
        )
    }
    Ok(())
}

fn emit_packages_not_found_within_workspace(
    ws: &Workspace<'_>,
    packages: &[String],
) -> CargoResult<()> {
    let (mut patterns, mut ids) = opt_patterns_and_ids(packages)?;
    let _: Vec<_> = ws
        .members()
        .filter(|pkg| {
            let id = ids.iter().find(|id| id.matches(pkg.package_id())).cloned();
            if let Some(id) = &id {
                ids.remove(id);
            }
            !id.is_some() && !match_patterns(pkg, &mut patterns)
        })
        .map(Package::package_id)
        .map(|id| id.to_spec())
        .collect();
    let names = ids
        .into_iter()
        .map(|id| id.to_string())
        .collect::<BTreeSet<_>>();
    emit_package_not_found(ws, names, false)?;
    emit_pattern_not_found(ws, patterns, false)?;
    Ok(())
}

/// Given a list opt-in or opt-out package selection strings, generates two
/// collections that represent glob patterns and package id specs respectively.
fn opt_patterns_and_ids(
    opt: &[String],
) -> CargoResult<(Vec<(glob::Pattern, bool)>, BTreeSet<PackageIdSpec>)> {
    let mut opt_patterns = Vec::new();
    let mut opt_ids = BTreeSet::new();
    for x in opt.iter() {
        match PackageIdSpec::parse(x) {
            Ok(spec) => {
                opt_ids.insert(spec);
            }
            Err(_) if is_glob_pattern(x) => opt_patterns.push((build_glob(x)?, false)),
            Err(e) => return Err(e.into()),
        }
    }
    Ok((opt_patterns, opt_ids))
}

/// Checks whether a package matches any of a list of glob patterns generated
/// from `opt_patterns_and_names`.
fn match_patterns(pkg: &Package, patterns: &mut Vec<(glob::Pattern, bool)>) -> bool {
    patterns.iter_mut().any(|(m, matched)| {
        let is_matched = m.matches(pkg.name().as_str());
        *matched |= is_matched;
        is_matched
    })
}

/// Build [`glob::Pattern`] with informative context.
pub fn build_glob(pat: &str) -> CargoResult<glob::Pattern> {
    glob::Pattern::new(pat).with_context(|| format!("cannot build glob pattern from `{}`", pat))
}
