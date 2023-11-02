use std::collections::{BTreeMap, HashMap, HashSet};

use tracing::trace;

use super::types::ConflictMap;
use crate::core::resolver::Context;
use crate::core::{Dependency, PackageId};

/// This is a trie for storing a large number of sets designed to
/// efficiently see if any of the stored sets are a subset of a search set.
enum ConflictStoreTrie {
    /// One of the stored sets.
    Leaf(ConflictMap),
    /// A map from an element to a subtrie where
    /// all the sets in the subtrie contains that element.
    Node(BTreeMap<PackageId, ConflictStoreTrie>),
}

impl ConflictStoreTrie {
    /// Finds any known set of conflicts, if any,
    /// where all elements return some from `is_active` and contain `PackageId` specified.
    /// If more than one are activated, then it will return
    /// one that will allow for the most jump-back.
    fn find(
        &self,
        is_active: &impl Fn(PackageId) -> Option<usize>,
        must_contain: Option<PackageId>,
        mut max_age: usize,
    ) -> Option<(&ConflictMap, usize)> {
        match self {
            ConflictStoreTrie::Leaf(c) => {
                if must_contain.is_none() {
                    Some((c, 0))
                } else {
                    // We did not find `must_contain`, so we need to keep looking.
                    None
                }
            }
            ConflictStoreTrie::Node(m) => {
                let mut out = None;
                for (&pid, store) in must_contain
                    .map(|f| m.range(..=f))
                    .unwrap_or_else(|| m.range(..))
                {
                    // If the key is active, then we need to check all of the corresponding subtrie.
                    if let Some(age_this) = is_active(pid) {
                        if age_this >= max_age && must_contain != Some(pid) {
                            // not worth looking at, it is to old.
                            continue;
                        }
                        if let Some((o, age_o)) =
                            store.find(is_active, must_contain.filter(|&f| f != pid), max_age)
                        {
                            let age = if must_contain == Some(pid) {
                                // all the results will include `must_contain`
                                // so the age of must_contain is not relevant to find the best result.
                                age_o
                            } else {
                                std::cmp::max(age_this, age_o)
                            };
                            if max_age > age {
                                // we found one that can jump-back further so replace the out.
                                out = Some((o, age));
                                // and don't look at anything older
                                max_age = age
                            }
                        }
                    }
                    // Else, if it is not active then there is no way any of the corresponding
                    // subtrie will be conflicting.
                }
                out
            }
        }
    }

    fn insert(&mut self, mut iter: impl Iterator<Item = PackageId>, con: ConflictMap) {
        if let Some(pid) = iter.next() {
            if let ConflictStoreTrie::Node(p) = self {
                p.entry(pid)
                    .or_insert_with(|| ConflictStoreTrie::Node(BTreeMap::new()))
                    .insert(iter, con);
            }
        // Else, we already have a subset of this in the `ConflictStore`.
        } else {
            // We are at the end of the set we are adding, there are three cases for what to do
            // next:
            // 1. `self` is an empty dummy Node inserted by `or_insert_with`
            //      in witch case we should replace it with `Leaf(con)`.
            // 2. `self` is a `Node` because we previously inserted a superset of
            //      the thing we are working on (I don't know if this happens in practice)
            //      but the subset that we are working on will
            //      always match any time the larger set would have
            //      in witch case we can replace it with `Leaf(con)`.
            // 3. `self` is a `Leaf` that is in the same spot in the structure as
            //      the thing we are working on. So it is equivalent.
            //      We can replace it with `Leaf(con)`.
            if cfg!(debug_assertions) {
                if let ConflictStoreTrie::Leaf(c) = self {
                    let a: Vec<_> = con.keys().collect();
                    let b: Vec<_> = c.keys().collect();
                    assert_eq!(a, b);
                }
            }
            *self = ConflictStoreTrie::Leaf(con)
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
    // look up which entries are still active without
    // linearly scanning through the full list.
    //
    // Also, as a final note, this map is **not** ever removed from. This remains
    // as a global cache which we never delete from. Any entry in this map is
    // unconditionally true regardless of our resolution history of how we got
    // here.
    con_from_dep: HashMap<Dependency, ConflictStoreTrie>,
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
    pub fn find(
        &self,
        dep: &Dependency,
        is_active: &impl Fn(PackageId) -> Option<usize>,
        must_contain: Option<PackageId>,
        max_age: usize,
    ) -> Option<&ConflictMap> {
        self.con_from_dep
            .get(dep)?
            .find(is_active, must_contain, max_age)
            .map(|(c, _)| c)
    }
    /// Finds any known set of conflicts, if any,
    /// which are activated in `cx` and contain `PackageId` specified.
    /// If more than one are activated, then it will return
    /// one that will allow for the most jump-back.
    pub fn find_conflicting(
        &self,
        cx: &Context,
        dep: &Dependency,
        must_contain: Option<PackageId>,
    ) -> Option<&ConflictMap> {
        let out = self.find(dep, &|id| cx.is_active(id), must_contain, usize::MAX);
        if cfg!(debug_assertions) {
            if let Some(c) = &out {
                assert!(cx.is_conflicting(None, c).is_some());
                if let Some(f) = must_contain {
                    assert!(c.contains_key(&f));
                }
            }
        }
        out
    }
    pub fn conflicting(&self, cx: &Context, dep: &Dependency) -> Option<&ConflictMap> {
        self.find_conflicting(cx, dep, None)
    }

    /// Adds to the cache a conflict of the form:
    /// `dep` is known to be unresolvable if
    /// all the `PackageId` entries are activated.
    pub fn insert(&mut self, dep: &Dependency, con: &ConflictMap) {
        if con.values().any(|c| c.is_public_dependency()) {
            // TODO: needs more info for back jumping
            // for now refuse to cache it.
            return;
        }
        self.con_from_dep
            .entry(dep.clone())
            .or_insert_with(|| ConflictStoreTrie::Node(BTreeMap::new()))
            .insert(con.keys().cloned(), con.clone());

        trace!(
            "{} = \"{}\" adding a skip {:?}",
            dep.package_name(),
            dep.version_req(),
            con
        );

        for c in con.keys() {
            self.dep_from_pid
                .entry(*c)
                .or_insert_with(HashSet::new)
                .insert(dep.clone());
        }
    }

    pub fn dependencies_conflicting_with(&self, pid: PackageId) -> Option<&HashSet<Dependency>> {
        self.dep_from_pid.get(&pid)
    }
}
