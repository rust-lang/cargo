use serde::{Serialize, Serializer};

use std::fmt;
use std::sync::RwLock;
use std::collections::HashSet;
use std::slice;
use std::str;
use std::mem;
use std::ptr;
use std::cmp::Ordering;
use std::ops::Deref;
use std::hash::{Hash, Hasher};

pub fn leak(s: String) -> &'static str {
    let boxed = s.into_boxed_str();
    let ptr = boxed.as_ptr();
    let len = boxed.len();
    mem::forget(boxed);
    unsafe {
        let slice = slice::from_raw_parts(ptr, len);
        str::from_utf8_unchecked(slice)
    }
}

lazy_static! {
    static ref STRING_CACHE: RwLock<HashSet<&'static str>> =
        RwLock::new(HashSet::new());
}

#[derive(Clone, Copy)]
pub struct InternedString {
    inner: &'static str,
}

impl PartialEq for InternedString {
    fn eq(&self, other: &InternedString) -> bool {
        ptr::eq(self.as_str(), other.as_str())
    }
}

impl Eq for InternedString {}

impl InternedString {
    pub fn new(str: &str) -> InternedString {
        let mut cache = STRING_CACHE.write().unwrap();
        let s = cache.get(str).map(|&s| s).unwrap_or_else(|| {
            let s = leak(str.to_string());
            cache.insert(s);
            s
        });

        InternedString { inner: s }
    }

    pub fn as_str(&self) -> &'static str {
        self.inner
    }
}

impl Deref for InternedString {
    type Target = str;

    fn deref(&self) -> &'static str {
        self.as_str()
    }
}

impl Hash for InternedString {
    // NB: we can't implement this as `identity(self).hash(state)`,
    // because we use this for on-disk fingerprints and so need
    // stability across Cargo invocations.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl fmt::Debug for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl Ord for InternedString {
    fn cmp(&self, other: &InternedString) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for InternedString {
    fn partial_cmp(&self, other: &InternedString) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for InternedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.inner)
    }
}
