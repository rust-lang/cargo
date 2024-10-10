use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU64;
use std::sync::{Mutex, OnceLock};

use crate::core::SourceId;
use crate::util::interning::InternedString;

static ACTIVATION_KEY_CACHE: OnceLock<Mutex<HashSet<&'static ActivationKeyInner>>> =
    OnceLock::new();

type ActivationKeyInner = (InternedString, SourceId, SemverCompatibility);

/// The activated version of a crate is based on the name, source, and semver compatibility.
#[derive(Clone, Copy, Eq)]
pub struct ActivationKey {
    inner: &'static ActivationKeyInner,
}

impl From<ActivationKeyInner> for ActivationKey {
    fn from(inner: ActivationKeyInner) -> Self {
        let mut cache = ACTIVATION_KEY_CACHE
            .get_or_init(|| Default::default())
            .lock()
            .unwrap();
        let inner = cache.get(&inner).cloned().unwrap_or_else(|| {
            let inner = Box::leak(Box::new(inner));
            cache.insert(inner);
            inner
        });
        Self { inner }
    }
}

impl ActivationKey {
    /// This function is used for the `Eq` and `Hash` impls to implement a "no hash" hashable value.
    /// This is possible since all `ActivationKey` are already interned in a `HashSet`.
    fn key(&self) -> u64 {
        std::ptr::from_ref(self.inner) as u64
    }
}

impl PartialEq for ActivationKey {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl nohash_hasher::IsEnabled for ActivationKey {}

impl Hash for ActivationKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.key());
    }
}

/// A type that represents when cargo treats two versions as compatible.
/// Versions `a` and `b` are compatible if their left-most nonzero digit is the same.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, PartialOrd, Ord)]
pub enum SemverCompatibility {
    Major(NonZeroU64),
    Minor(NonZeroU64),
    Patch(u64),
}

impl From<&semver::Version> for SemverCompatibility {
    fn from(ver: &semver::Version) -> Self {
        if let Some(m) = NonZeroU64::new(ver.major) {
            return SemverCompatibility::Major(m);
        }
        if let Some(m) = NonZeroU64::new(ver.minor) {
            return SemverCompatibility::Minor(m);
        }
        SemverCompatibility::Patch(ver.patch)
    }
}
