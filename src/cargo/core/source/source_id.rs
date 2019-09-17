use std::cmp::{self, Ordering};
use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::hash::{self, Hash};
use std::path::Path;
use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Mutex;

use log::trace;
use serde::de;
use serde::ser;
use url::Url;

use crate::core::PackageId;
use crate::ops;
use crate::sources::DirectorySource;
use crate::sources::{GitSource, PathSource, RegistrySource, CRATES_IO_INDEX};
use crate::util::{CanonicalUrl, CargoResult, Config, IntoUrl};

lazy_static::lazy_static! {
    static ref SOURCE_ID_CACHE: Mutex<HashSet<&'static SourceIdInner>> = Mutex::new(HashSet::new());
}

/// Unique identifier for a source of packages.
#[derive(Clone, Copy, Eq, Debug)]
pub struct SourceId {
    inner: &'static SourceIdInner,
}

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
struct SourceIdInner {
    /// The source URL.
    url: Url,
    /// The canonical version of the above url
    canonical_url: CanonicalUrl,
    /// The source kind.
    kind: Kind,
    /// For example, the exact Git revision of the specified branch for a Git Source.
    precise: Option<String>,
    /// Name of the registry source for alternative registries
    /// WARNING: this is not always set for alt-registries when the name is
    /// not known.
    name: Option<String>,
}

/// The possible kinds of code source. Along with `SourceIdInner`, this fully defines the
/// source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    /// A git repository.
    Git(GitReference),
    /// A local path..
    Path,
    /// A remote registry.
    Registry,
    /// A local filesystem-based registry.
    LocalRegistry,
    /// A directory-based registry.
    Directory,
}

/// Information to find a specific commit in a Git repository.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitReference {
    /// From a tag.
    Tag(String),
    /// From the HEAD of a branch.
    Branch(String),
    /// From a specific revision.
    Rev(String),
}

impl SourceId {
    /// Creates a `SourceId` object from the kind and URL.
    ///
    /// The canonical url will be calculated, but the precise field will not
    fn new(kind: Kind, url: Url) -> CargoResult<SourceId> {
        let source_id = SourceId::wrap(SourceIdInner {
            kind,
            canonical_url: CanonicalUrl::new(&url)?,
            url,
            precise: None,
            name: None,
        });
        Ok(source_id)
    }

    fn wrap(inner: SourceIdInner) -> SourceId {
        let mut cache = SOURCE_ID_CACHE.lock().unwrap();
        let inner = cache.get(&inner).cloned().unwrap_or_else(|| {
            let inner = Box::leak(Box::new(inner));
            cache.insert(inner);
            inner
        });
        SourceId { inner }
    }

    /// Parses a source URL and returns the corresponding ID.
    ///
    /// ## Example
    ///
    /// ```
    /// use cargo::core::SourceId;
    /// SourceId::from_url("git+https://github.com/alexcrichton/\
    ///                     libssh2-static-sys#80e71a3021618eb05\
    ///                     656c58fb7c5ef5f12bc747f");
    /// ```
    pub fn from_url(string: &str) -> CargoResult<SourceId> {
        let mut parts = string.splitn(2, '+');
        let kind = parts.next().unwrap();
        let url = parts
            .next()
            .ok_or_else(|| failure::format_err!("invalid source `{}`", string))?;

        match kind {
            "git" => {
                let mut url = url.into_url()?;
                let mut reference = GitReference::Branch("master".to_string());
                for (k, v) in url.query_pairs() {
                    match &k[..] {
                        // Map older 'ref' to branch.
                        "branch" | "ref" => reference = GitReference::Branch(v.into_owned()),

                        "rev" => reference = GitReference::Rev(v.into_owned()),
                        "tag" => reference = GitReference::Tag(v.into_owned()),
                        _ => {}
                    }
                }
                let precise = url.fragment().map(|s| s.to_owned());
                url.set_fragment(None);
                url.set_query(None);
                Ok(SourceId::for_git(&url, reference)?.with_precise(precise))
            }
            "registry" => {
                let url = url.into_url()?;
                Ok(SourceId::new(Kind::Registry, url)?.with_precise(Some("locked".to_string())))
            }
            "path" => {
                let url = url.into_url()?;
                SourceId::new(Kind::Path, url)
            }
            kind => Err(failure::format_err!(
                "unsupported source protocol: {}",
                kind
            )),
        }
    }

    /// A view of the `SourceId` that can be `Display`ed as a URL.
    pub fn into_url(&self) -> SourceIdIntoUrl<'_> {
        SourceIdIntoUrl {
            inner: &*self.inner,
        }
    }

    /// Creates a `SourceId` from a filesystem path.
    ///
    /// `path`: an absolute path.
    pub fn for_path(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(Kind::Path, url)
    }

    /// Creates a `SourceId` from a Git reference.
    pub fn for_git(url: &Url, reference: GitReference) -> CargoResult<SourceId> {
        SourceId::new(Kind::Git(reference), url.clone())
    }

    /// Creates a SourceId from a registry URL.
    pub fn for_registry(url: &Url) -> CargoResult<SourceId> {
        SourceId::new(Kind::Registry, url.clone())
    }

    /// Creates a SourceId from a local registry path.
    pub fn for_local_registry(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(Kind::LocalRegistry, url)
    }

    /// Creates a `SourceId` from a directory path.
    pub fn for_directory(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(Kind::Directory, url)
    }

    /// Returns the `SourceId` corresponding to the main repository.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a `.cargo/config`.
    pub fn crates_io(config: &Config) -> CargoResult<SourceId> {
        config.crates_io_source_id(|| {
            let cfg = ops::registry_configuration(config, None)?;
            let url = if let Some(ref index) = cfg.index {
                static WARNED: AtomicBool = AtomicBool::new(false);
                if !WARNED.swap(true, SeqCst) {
                    config.shell().warn(
                        "custom registry support via \
                         the `registry.index` configuration is \
                         being removed, this functionality \
                         will not work in the future",
                    )?;
                }
                &index[..]
            } else {
                CRATES_IO_INDEX
            };
            let url = url.into_url()?;
            SourceId::for_registry(&url)
        })
    }

    pub fn alt_registry(config: &Config, key: &str) -> CargoResult<SourceId> {
        let url = config.get_registry_index(key)?;
        Ok(SourceId::wrap(SourceIdInner {
            kind: Kind::Registry,
            canonical_url: CanonicalUrl::new(&url)?,
            url,
            precise: None,
            name: Some(key.to_string()),
        }))
    }

    /// Gets this source URL.
    pub fn url(&self) -> &Url {
        &self.inner.url
    }

    /// Gets the canonical URL of this source, used for internal comparison
    /// purposes.
    pub fn canonical_url(&self) -> &CanonicalUrl {
        &self.inner.canonical_url
    }

    pub fn display_index(self) -> String {
        if self.is_default_registry() {
            "crates.io index".to_string()
        } else {
            format!("`{}` index", url_display(self.url()))
        }
    }

    pub fn display_registry_name(self) -> String {
        if self.is_default_registry() {
            "crates.io".to_string()
        } else if let Some(name) = &self.inner.name {
            name.clone()
        } else {
            url_display(self.url())
        }
    }

    /// Returns `true` if this source is from a filesystem path.
    pub fn is_path(self) -> bool {
        self.inner.kind == Kind::Path
    }

    /// Returns `true` if this source is from a registry (either local or not).
    pub fn is_registry(self) -> bool {
        match self.inner.kind {
            Kind::Registry | Kind::LocalRegistry => true,
            _ => false,
        }
    }

    /// Returns `true` if this source is a "remote" registry.
    ///
    /// "remote" may also mean a file URL to a git index, so it is not
    /// necessarily "remote". This just means it is not `local-registry`.
    pub fn is_remote_registry(self) -> bool {
        match self.inner.kind {
            Kind::Registry => true,
            _ => false,
        }
    }

    /// Returns `true` if this source from a Git repository.
    pub fn is_git(self) -> bool {
        match self.inner.kind {
            Kind::Git(_) => true,
            _ => false,
        }
    }

    /// Creates an implementation of `Source` corresponding to this ID.
    pub fn load<'a>(
        self,
        config: &'a Config,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn super::Source + 'a>> {
        trace!("loading SourceId; {}", self);
        match self.inner.kind {
            Kind::Git(..) => Ok(Box::new(GitSource::new(self, config)?)),
            Kind::Path => {
                let path = match self.inner.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => panic!("path sources cannot be remote"),
                };
                Ok(Box::new(PathSource::new(&path, self, config)))
            }
            Kind::Registry => Ok(Box::new(RegistrySource::remote(
                self,
                yanked_whitelist,
                config,
            ))),
            Kind::LocalRegistry => {
                let path = match self.inner.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => panic!("path sources cannot be remote"),
                };
                Ok(Box::new(RegistrySource::local(
                    self,
                    &path,
                    yanked_whitelist,
                    config,
                )))
            }
            Kind::Directory => {
                let path = match self.inner.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => panic!("path sources cannot be remote"),
                };
                Ok(Box::new(DirectorySource::new(&path, self, config)))
            }
        }
    }

    /// Gets the value of the precise field.
    pub fn precise(self) -> Option<&'static str> {
        self.inner.precise.as_ref().map(|s| &s[..])
    }

    /// Gets the Git reference if this is a git source, otherwise `None`.
    pub fn git_reference(self) -> Option<&'static GitReference> {
        match self.inner.kind {
            Kind::Git(ref s) => Some(s),
            _ => None,
        }
    }

    /// Creates a new `SourceId` from this source with the given `precise`.
    pub fn with_precise(self, v: Option<String>) -> SourceId {
        SourceId::wrap(SourceIdInner {
            precise: v,
            ..(*self.inner).clone()
        })
    }

    /// Returns `true` if the remote registry is the standard <https://crates.io>.
    pub fn is_default_registry(self) -> bool {
        match self.inner.kind {
            Kind::Registry => {}
            _ => return false,
        }
        self.inner.url.as_str() == CRATES_IO_INDEX
    }

    /// Hashes `self`.
    ///
    /// For paths, remove the workspace prefix so the same source will give the
    /// same hash in different locations.
    pub fn stable_hash<S: hash::Hasher>(self, workspace: &Path, into: &mut S) {
        if self.is_path() {
            if let Ok(p) = self
                .inner
                .url
                .to_file_path()
                .unwrap()
                .strip_prefix(workspace)
            {
                self.inner.kind.hash(into);
                p.to_str().unwrap().hash(into);
                return;
            }
        }
        self.hash(into)
    }

    pub fn full_eq(self, other: SourceId) -> bool {
        ptr::eq(self.inner, other.inner)
    }

    pub fn full_hash<S: hash::Hasher>(self, into: &mut S) {
        ptr::NonNull::from(self.inner).hash(into)
    }
}

impl PartialOrd for SourceId {
    fn partial_cmp(&self, other: &SourceId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceId {
    fn cmp(&self, other: &SourceId) -> Ordering {
        self.inner.cmp(other.inner)
    }
}

impl ser::Serialize for SourceId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        if self.is_path() {
            None::<String>.serialize(s)
        } else {
            s.collect_str(&self.into_url())
        }
    }
}

impl<'de> de::Deserialize<'de> for SourceId {
    fn deserialize<D>(d: D) -> Result<SourceId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;
        SourceId::from_url(&string).map_err(de::Error::custom)
    }
}

fn url_display(url: &Url) -> String {
    if url.scheme() == "file" {
        if let Ok(path) = url.to_file_path() {
            if let Some(path_str) = path.to_str() {
                return path_str.to_string();
            }
        }
    }

    url.as_str().to_string()
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.inner.kind {
            Kind::Git(ref reference) => {
                // Don't replace the URL display for git references,
                // because those are kind of expected to be URLs.
                write!(f, "{}", self.inner.url)?;
                if let Some(pretty) = reference.pretty_ref() {
                    write!(f, "?{}", pretty)?;
                }

                if let Some(ref s) = self.inner.precise {
                    let len = cmp::min(s.len(), 8);
                    write!(f, "#{}", &s[..len])?;
                }
                Ok(())
            }
            Kind::Path => write!(f, "{}", url_display(&self.inner.url)),
            Kind::Registry => write!(f, "registry `{}`", url_display(&self.inner.url)),
            Kind::LocalRegistry => write!(f, "registry `{}`", url_display(&self.inner.url)),
            Kind::Directory => write!(f, "dir {}", url_display(&self.inner.url)),
        }
    }
}

// Custom equality defined as canonical URL equality for git sources and
// URL equality for other sources, ignoring the `precise` and `name` fields.
impl PartialEq for SourceId {
    fn eq(&self, other: &SourceId) -> bool {
        if ptr::eq(self.inner, other.inner) {
            return true;
        }
        if self.inner.kind != other.inner.kind {
            return false;
        }
        if self.inner.url == other.inner.url {
            return true;
        }

        match (&self.inner.kind, &other.inner.kind) {
            (Kind::Git(ref1), Kind::Git(ref2)) => {
                ref1 == ref2 && self.inner.canonical_url == other.inner.canonical_url
            }
            _ => false,
        }
    }
}

impl PartialOrd for SourceIdInner {
    fn partial_cmp(&self, other: &SourceIdInner) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceIdInner {
    fn cmp(&self, other: &SourceIdInner) -> Ordering {
        match self.kind.cmp(&other.kind) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.url.cmp(&other.url) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match (&self.kind, &other.kind) {
            (Kind::Git(ref1), Kind::Git(ref2)) => {
                (ref1, &self.canonical_url).cmp(&(ref2, &other.canonical_url))
            }
            _ => self.kind.cmp(&other.kind),
        }
    }
}

// The hash of SourceId is used in the name of some Cargo folders, so shouldn't
// vary. `as_str` gives the serialisation of a url (which has a spec) and so
// insulates against possible changes in how the url crate does hashing.
impl Hash for SourceId {
    fn hash<S: hash::Hasher>(&self, into: &mut S) {
        self.inner.kind.hash(into);
        match self.inner.kind {
            Kind::Git(_) => self.inner.canonical_url.hash(into),
            _ => self.inner.url.as_str().hash(into),
        }
    }
}

/// A `Display`able view into a `SourceId` that will write it as a url
pub struct SourceIdIntoUrl<'a> {
    inner: &'a SourceIdInner,
}

impl<'a> fmt::Display for SourceIdIntoUrl<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self.inner {
            SourceIdInner {
                kind: Kind::Path,
                ref url,
                ..
            } => write!(f, "path+{}", url),
            SourceIdInner {
                kind: Kind::Git(ref reference),
                ref url,
                ref precise,
                ..
            } => {
                write!(f, "git+{}", url)?;
                if let Some(pretty) = reference.pretty_ref() {
                    write!(f, "?{}", pretty)?;
                }
                if let Some(precise) = precise.as_ref() {
                    write!(f, "#{}", precise)?;
                }
                Ok(())
            }
            SourceIdInner {
                kind: Kind::Registry,
                ref url,
                ..
            } => write!(f, "registry+{}", url),
            SourceIdInner {
                kind: Kind::LocalRegistry,
                ref url,
                ..
            } => write!(f, "local-registry+{}", url),
            SourceIdInner {
                kind: Kind::Directory,
                ref url,
                ..
            } => write!(f, "directory+{}", url),
        }
    }
}

impl GitReference {
    /// Returns a `Display`able view of this git reference, or None if using
    /// the head of the "master" branch
    pub fn pretty_ref(&self) -> Option<PrettyRef<'_>> {
        match *self {
            GitReference::Branch(ref s) if *s == "master" => None,
            _ => Some(PrettyRef { inner: self }),
        }
    }
}

/// A git reference that can be `Display`ed
pub struct PrettyRef<'a> {
    inner: &'a GitReference,
}

impl<'a> fmt::Display for PrettyRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self.inner {
            GitReference::Branch(ref b) => write!(f, "branch={}", b),
            GitReference::Tag(ref s) => write!(f, "tag={}", s),
            GitReference::Rev(ref s) => write!(f, "rev={}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GitReference, Kind, SourceId};
    use crate::util::IntoUrl;

    #[test]
    fn github_sources_equal() {
        let loc = "https://github.com/foo/bar".into_url().unwrap();
        let master = Kind::Git(GitReference::Branch("master".to_string()));
        let s1 = SourceId::new(master.clone(), loc).unwrap();

        let loc = "git://github.com/foo/bar".into_url().unwrap();
        let s2 = SourceId::new(master, loc.clone()).unwrap();

        assert_eq!(s1, s2);

        let foo = Kind::Git(GitReference::Branch("foo".to_string()));
        let s3 = SourceId::new(foo, loc).unwrap();
        assert_ne!(s1, s3);
    }
}
