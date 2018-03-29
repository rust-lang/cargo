use std::collections::{HashMap, HashSet};

use core::{Dependency, PackageId};
use core::resolver::Context;
use super::types::ConflictReason;

pub(super) struct ConflictCache {
    // `con_from_dep` is a cache of the reasons for each time we
    // backtrack. For example after several backtracks we may have:
    //
    //  con_from_dep[`foo = "^1.0.2"`] = vec![
    //      map!{`foo=1.0.1`: Semver},
    //      map!{`foo=1.0.0`: Semver},
    //  ];
    //
    // This can be read as "we cannot find a candidate for dep `foo = "^1.0.2"`
    // if either `foo=1.0.1` OR `foo=1.0.0` are activated".
    //
    // Another example after several backtracks we may have:
    //
    //  con_from_dep[`foo = ">=0.8.2, <=0.9.3"`] = vec![
    //      map!{`foo=0.8.1`: Semver, `foo=0.9.4`: Semver},
    //  ];
    //
    // This can be read as "we cannot find a candidate for dep `foo = ">=0.8.2,
    // <=0.9.3"` if both `foo=0.8.1` AND `foo=0.9.4` are activated".
    //
    // This is used to make sure we don't queue work we know will fail. See the
    // discussion in https://github.com/rust-lang/cargo/pull/5168 for why this
    // is so important, and there can probably be a better data structure here
    // but for now this works well enough!
    //
    // Also, as a final note, this map is *not* ever removed from. This remains
    // as a global cache which we never delete from. Any entry in this map is
    // unconditionally true regardless of our resolution history of how we got
    // here.
    con_from_dep: HashMap<Dependency, Vec<HashMap<PackageId, ConflictReason>>>,
    // `past_conflict_triggers` is an
    // of `past_conflicting_activations`.
    // For every `PackageId` this lists the `Dependency`s that mention it in `past_conflicting_activations`.
    dep_from_pid: HashMap<PackageId, HashSet<Dependency>>,
}

impl ConflictCache {
    pub fn new() -> ConflictCache {
        ConflictCache {
            con_from_dep: HashMap::new(),
            dep_from_pid: HashMap::new(),
        }
    }
    /// Finds any known set of conflicts, if any,
    /// which are activated in `cx` and pass the `filter` specified?
    pub fn find_conflicting<F>(
        &self,
        cx: &Context,
        dep: &Dependency,
        filter: F,
    ) -> Option<&HashMap<PackageId, ConflictReason>>
    where
        for<'r> F: FnMut(&'r &HashMap<PackageId, ConflictReason>) -> bool,
    {
        self.con_from_dep
            .get(dep)?
            .iter()
            .filter(filter)
            .find(|conflicting| cx.is_conflicting(None, conflicting))
    }
    pub fn conflicting(
        &self,
        cx: &Context,
        dep: &Dependency,
    ) -> Option<&HashMap<PackageId, ConflictReason>> {
        self.find_conflicting(cx, dep, |_| true)
    }

    /// Add to the cache a conflict of the form:
    /// `dep` is known to be unresolvable if
    /// all the `PackageId` entries are activated
    pub fn insert(&mut self, dep: &Dependency, con: &HashMap<PackageId, ConflictReason>) {
        let past = self.con_from_dep
            .entry(dep.clone())
            .or_insert_with(Vec::new);
        if !past.contains(con) {
            trace!("{} adding a skip {:?}", dep.name(), con);
            past.push(con.clone());
            for c in con.keys() {
                self.dep_from_pid
                    .entry(c.clone())
                    .or_insert_with(HashSet::new)
                    .insert(dep.clone());
            }
        }
    }
    pub fn dependencies_conflicting_with(&self, pid: &PackageId) -> Option<&HashSet<Dependency>> {
        self.dep_from_pid.get(pid)
    }
}
