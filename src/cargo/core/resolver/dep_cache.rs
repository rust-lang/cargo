use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use log::debug;

use crate::core::{Dependency, PackageId, PackageIdSpec, Registry};
use crate::util::errors::CargoResult;

use crate::core::resolver::types::Candidate;

pub struct RegistryQueryer<'a> {
    pub registry: &'a mut (dyn Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    try_to_use: &'a HashSet<PackageId>,
    // If set the list of dependency candidates will be sorted by minimal
    // versions first. That allows `cargo update -Z minimal-versions` which will
    // specify minimum dependency versions to be used.
    minimal_versions: bool,
    cache: HashMap<Dependency, Rc<Vec<Candidate>>>,
    used_replacements: HashMap<PackageId, PackageId>,
}

impl<'a> RegistryQueryer<'a> {
    pub fn new(
        registry: &'a mut dyn Registry,
        replacements: &'a [(PackageIdSpec, Dependency)],
        try_to_use: &'a HashSet<PackageId>,
        minimal_versions: bool,
    ) -> Self {
        RegistryQueryer {
            registry,
            replacements,
            try_to_use,
            minimal_versions,
            cache: HashMap::new(),
            used_replacements: HashMap::new(),
        }
    }

    pub fn used_replacement_for(&self, p: PackageId) -> Option<(PackageId, PackageId)> {
        self.used_replacements.get(&p).map(|&r| (p, r))
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    pub fn query(&mut self, dep: &Dependency) -> CargoResult<Rc<Vec<Candidate>>> {
        if let Some(out) = self.cache.get(dep).cloned() {
            return Ok(out);
        }

        let mut ret = Vec::new();
        self.registry.query(
            dep,
            &mut |s| {
                ret.push(Candidate {
                    summary: s,
                    replace: None,
                });
            },
            false,
        )?;
        for candidate in ret.iter_mut() {
            let summary = &candidate.summary;

            let mut potential_matches = self
                .replacements
                .iter()
                .filter(|&&(ref spec, _)| spec.matches(summary.package_id()));

            let &(ref spec, ref dep) = match potential_matches.next() {
                None => continue,
                Some(replacement) => replacement,
            };
            debug!(
                "found an override for {} {}",
                dep.package_name(),
                dep.version_req()
            );

            let mut summaries = self.registry.query_vec(dep, false)?.into_iter();
            let s = summaries.next().ok_or_else(|| {
                failure::format_err!(
                    "no matching package for override `{}` found\n\
                     location searched: {}\n\
                     version required: {}",
                    spec,
                    dep.source_id(),
                    dep.version_req()
                )
            })?;
            let summaries = summaries.collect::<Vec<_>>();
            if !summaries.is_empty() {
                let bullets = summaries
                    .iter()
                    .map(|s| format!("  * {}", s.package_id()))
                    .collect::<Vec<_>>();
                failure::bail!(
                    "the replacement specification `{}` matched \
                     multiple packages:\n  * {}\n{}",
                    spec,
                    s.package_id(),
                    bullets.join("\n")
                );
            }

            // The dependency should be hard-coded to have the same name and an
            // exact version requirement, so both of these assertions should
            // never fail.
            assert_eq!(s.version(), summary.version());
            assert_eq!(s.name(), summary.name());

            let replace = if s.source_id() == summary.source_id() {
                debug!("Preventing\n{:?}\nfrom replacing\n{:?}", summary, s);
                None
            } else {
                Some(s)
            };
            let matched_spec = spec.clone();

            // Make sure no duplicates
            if let Some(&(ref spec, _)) = potential_matches.next() {
                failure::bail!(
                    "overlapping replacement specifications found:\n\n  \
                     * {}\n  * {}\n\nboth specifications match: {}",
                    matched_spec,
                    spec,
                    summary.package_id()
                );
            }

            for dep in summary.dependencies() {
                debug!("\t{} => {}", dep.package_name(), dep.version_req());
            }
            if let Some(r) = &replace {
                self.used_replacements
                    .insert(summary.package_id(), r.package_id());
            }

            candidate.replace = replace;
        }

        // When we attempt versions for a package we'll want to do so in a
        // sorted fashion to pick the "best candidates" first. Currently we try
        // prioritized summaries (those in `try_to_use`) and failing that we
        // list everything from the maximum version to the lowest version.
        ret.sort_unstable_by(|a, b| {
            let a_in_previous = self.try_to_use.contains(&a.summary.package_id());
            let b_in_previous = self.try_to_use.contains(&b.summary.package_id());
            let previous_cmp = a_in_previous.cmp(&b_in_previous).reverse();
            match previous_cmp {
                Ordering::Equal => {
                    let cmp = a.summary.version().cmp(b.summary.version());
                    if self.minimal_versions {
                        // Lower version ordered first.
                        cmp
                    } else {
                        // Higher version ordered first.
                        cmp.reverse()
                    }
                }
                _ => previous_cmp,
            }
        });

        let out = Rc::new(ret);

        self.cache.insert(dep.clone(), out.clone());

        Ok(out)
    }
}
