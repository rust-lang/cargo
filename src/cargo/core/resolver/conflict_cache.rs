use std::collections::{BTreeMap, HashMap, HashSet};

use super::types::ConflictReason;
use core::resolver::Context;
use core::{Dependency, PackageId};

enum ConflictStore {
    Con(BTreeMap<PackageId, ConflictReason>),
    Map(HashMap<PackageId, ConflictStore>),
}

impl ConflictStore {
    /// Finds any known set of conflicts, if any,
    /// which are activated in `cx` and pass the `filter` specified?
    pub fn find_conflicting<F>(
        &self,
        cx: &Context,
        filter: &F,
    ) -> Option<&BTreeMap<PackageId, ConflictReason>>
    where
        for<'r> F: Fn(&'r &BTreeMap<PackageId, ConflictReason>) -> bool,
    {
        match self {
            ConflictStore::Con(c) => {
                if filter(&c) {
                    Some(c)
                } else {
                    None
                }
            }
            ConflictStore::Map(m) => {
                for (pid, store) in m {
                    if cx.is_active(pid) {
                        if let Some(o) = store.find_conflicting(cx, filter) {
                            return Some(o);
                        }
                    }
                }
                None
            }
        }
    }

    pub fn insert<'a>(
        &mut self,
        mut iter: impl Iterator<Item = &'a PackageId>,
        con: BTreeMap<PackageId, ConflictReason>,
    ) {
        if let Some(pid) = iter.next() {
            if let ConflictStore::Map(p) = self {
                p.entry(pid.clone())
                    .or_insert_with(|| ConflictStore::Map(HashMap::new()))
                    .insert(iter, con);
            }
            // else, We already have a subset of this in the ConflictStore
        } else {
            *self = ConflictStore::Con(con)
        }
    }
}

pub(super) struct ConflictCache {
    // `con_from_dep` is a cache of the reasons for each time we
    // backtrack. For example after several backtracks we may have:
    //
    //  con_from_dep[`foo = "^1.0.2"`] = map!{
    //      `foo=1.0.1`: map!{`foo=1.0.1`: Semver},
    //      `foo=1.0.0`: map!{`foo=1.0.0`: Semver},
    //  };
    //
    // This can be read as "we cannot find a candidate for dep `foo = "^1.0.2"`
    // if either `foo=1.0.1` OR `foo=1.0.0` are activated".
    //
    // Another example after several backtracks we may have:
    //
    //  con_from_dep[`foo = ">=0.8.2, <=0.9.3"`] = map!{
    //      `foo=0.8.1`: map!{
    //          `foo=0.9.4`: map!{`foo=0.8.1`: Semver, `foo=0.9.4`: Semver},
    //      }
    //  };
    //
    // This can be read as "we cannot find a candidate for dep `foo = ">=0.8.2,
    // <=0.9.3"` if both `foo=0.8.1` AND `foo=0.9.4` are activated".
    //
    // This is used to make sure we don't queue work we know will fail. See the
    // discussion in https://github.com/rust-lang/cargo/pull/5168 for why this
    // is so important. The nested HashMaps act as a kind of btree, that lets us
    // look up a witch entry's are still active without
    // linearly scanning through the full list.
    //
    // Also, as a final note, this map is *not* ever removed from. This remains
    // as a global cache which we never delete from. Any entry in this map is
    // unconditionally true regardless of our resolution history of how we got
    // here.
    con_from_dep: HashMap<Dependency, ConflictStore>,
    // `dep_from_pid` is an inverse-index of `con_from_dep`.
    // For every `PackageId` this lists the `Dependency`s that mention it in `dep_from_pid`.
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
    ) -> Option<&BTreeMap<PackageId, ConflictReason>>
    where
        for<'r> F: Fn(&'r &BTreeMap<PackageId, ConflictReason>) -> bool,
    {
        self.con_from_dep.get(dep)?.find_conflicting(cx, &filter)
    }
    pub fn conflicting(
        &self,
        cx: &Context,
        dep: &Dependency,
    ) -> Option<&BTreeMap<PackageId, ConflictReason>> {
        self.find_conflicting(cx, dep, |_| true)
    }

    /// Add to the cache a conflict of the form:
    /// `dep` is known to be unresolvable if
    /// all the `PackageId` entries are activated
    pub fn insert(&mut self, dep: &Dependency, con: &BTreeMap<PackageId, ConflictReason>) {
        self.con_from_dep
            .entry(dep.clone())
            .or_insert_with(|| ConflictStore::Map(HashMap::new()))
            .insert(con.keys(), con.clone());

        trace!(
            "{} = \"{}\" adding a skip {:?}",
            dep.package_name(),
            dep.version_req(),
            con
        );

        for c in con.keys() {
            self.dep_from_pid
                .entry(c.clone())
                .or_insert_with(HashSet::new)
                .insert(dep.clone());
        }
    }
    pub fn dependencies_conflicting_with(&self, pid: &PackageId) -> Option<&HashSet<Dependency>> {
        self.dep_from_pid.get(pid)
    }
}
