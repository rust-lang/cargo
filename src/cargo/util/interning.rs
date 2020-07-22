use serde::{Serialize, Serializer};
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::path::Path;
use std::ptr;
use std::str;
use std::sync::Mutex;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

lazy_static::lazy_static! {
    static ref STRING_CACHE: Mutex<HashSet<&'static str>> = Mutex::new(HashSet::new());
}

#[derive(Clone, Copy)]
pub struct InternedString {
    inner: &'static str,
}

impl<'a> From<&'a str> for InternedString {
    fn from(item: &'a str) -> Self {
        InternedString::new(item)
    }
}

impl<'a> From<&'a String> for InternedString {
    fn from(item: &'a String) -> Self {
        InternedString::new(item)
    }
}

impl From<String> for InternedString {
    fn from(item: String) -> Self {
        InternedString::new(&item)
    }
}

impl PartialEq for InternedString {
    fn eq(&self, other: &InternedString) -> bool {
        ptr::eq(self.as_str(), other.as_str())
    }
}

impl PartialEq<str> for InternedString {
    fn eq(&self, other: &str) -> bool {
        *self == other
    }
}

impl<'a> PartialEq<&'a str> for InternedString {
    fn eq(&self, other: &&str) -> bool {
        **self == **other
    }
}

impl Eq for InternedString {}

impl InternedString {
    pub fn new(str: &str) -> InternedString {
        let mut cache = STRING_CACHE.lock().unwrap();
        let s = cache.get(str).cloned().unwrap_or_else(|| {
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

impl AsRef<str> for InternedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<OsStr> for InternedString {
    fn as_ref(&self) -> &OsStr {
        self.as_str().as_ref()
    }
}

impl AsRef<Path> for InternedString {
    fn as_ref(&self) -> &Path {
        self.as_str().as_ref()
    }
}

impl Hash for InternedString {
    // N.B., we can't implement this as `identity(self).hash(state)`,
    // because we use this for on-disk fingerprints and so need
    // stability across Cargo invocations.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Borrow<str> for InternedString {
    // If we implement Hash as `identity(self).hash(state)`,
    // then this will need to be removed.
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

struct InternedStringVisitor;

impl<'de> serde::Deserialize<'de> for InternedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(InternedStringVisitor)
    }
}

impl<'de> serde::de::Visitor<'de> for InternedStringVisitor {
    type Value = InternedString;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an String like thing")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(InternedString::new(v))
    }
}
