use crate::core::PackageId;
use crate::sources::{DirectorySource, CRATES_IO_DOMAIN, CRATES_IO_INDEX};
use crate::sources::{GitSource, PathSource, RegistrySource};
use crate::util::{CanonicalUrl, CargoResult, Config, IntoUrl};
use log::trace;
use serde::de;
use serde::ser;
use std::cmp::{self, Ordering};
use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::hash::{self, Hash};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;
use url::Url;

lazy_static::lazy_static! {
    static ref SOURCE_ID_CACHE: Mutex<HashSet<&'static SourceIdInner>> = Default::default();
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
    kind: SourceKind,
    /// For example, the exact Git revision of the specified branch for a Git Source.
    precise: Option<String>,
    /// Name of the registry source for alternative registries
    /// WARNING: this is not always set for alt-registries when the name is
    /// not known.
    name: Option<String>,
}

/// The possible kinds of code source. Along with `SourceIdInner`, this fully defines the
/// source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SourceKind {
    /// A git repository.
    Git(GitReference),
    /// A local path.
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
    /// From a branch.
    Branch(String),
    /// From a specific revision.
    Rev(String),
    /// The default branch of the repository, the reference named `HEAD`.
    DefaultBranch,
}

impl SourceId {
    /// Creates a `SourceId` object from the kind and URL.
    ///
    /// The canonical url will be calculated, but the precise field will not
    fn new(kind: SourceKind, url: Url) -> CargoResult<SourceId> {
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
            .ok_or_else(|| anyhow::format_err!("invalid source `{}`", string))?;

        match kind {
            "git" => {
                let mut url = url.into_url()?;
                let mut reference = GitReference::DefaultBranch;
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
                Ok(SourceId::new(SourceKind::Registry, url)?
                    .with_precise(Some("locked".to_string())))
            }
            "path" => {
                let url = url.into_url()?;
                SourceId::new(SourceKind::Path, url)
            }
            kind => Err(anyhow::format_err!("unsupported source protocol: {}", kind)),
        }
    }

    /// A view of the `SourceId` that can be `Display`ed as a URL.
    pub fn as_url(&self) -> SourceIdAsUrl<'_> {
        SourceIdAsUrl {
            inner: &*self.inner,
        }
    }

    /// Creates a `SourceId` from a filesystem path.
    ///
    /// `path`: an absolute path.
    pub fn for_path(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(SourceKind::Path, url)
    }

    /// Creates a `SourceId` from a Git reference.
    pub fn for_git(url: &Url, reference: GitReference) -> CargoResult<SourceId> {
        SourceId::new(SourceKind::Git(reference), url.clone())
    }

    /// Creates a SourceId from a registry URL.
    pub fn for_registry(url: &Url) -> CargoResult<SourceId> {
        SourceId::new(SourceKind::Registry, url.clone())
    }

    /// Creates a SourceId from a local registry path.
    pub fn for_local_registry(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(SourceKind::LocalRegistry, url)
    }

    /// Creates a `SourceId` from a directory path.
    pub fn for_directory(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(SourceKind::Directory, url)
    }

    /// Returns the `SourceId` corresponding to the main repository.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a `.cargo/config`.
    pub fn crates_io(config: &Config) -> CargoResult<SourceId> {
        config.crates_io_source_id(|| {
            config.check_registry_index_not_set()?;
            let url = CRATES_IO_INDEX.into_url().unwrap();
            SourceId::for_registry(&url)
        })
    }

    pub fn alt_registry(config: &Config, key: &str) -> CargoResult<SourceId> {
        let url = config.get_registry_index(key)?;
        Ok(SourceId::wrap(SourceIdInner {
            kind: SourceKind::Registry,
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
            CRATES_IO_DOMAIN.to_string()
        } else if let Some(name) = &self.inner.name {
            name.clone()
        } else {
            url_display(self.url())
        }
    }

    /// Returns `true` if this source is from a filesystem path.
    pub fn is_path(self) -> bool {
        self.inner.kind == SourceKind::Path
    }

    /// Returns the local path if this is a path dependency.
    pub fn local_path(self) -> Option<PathBuf> {
        if self.inner.kind != SourceKind::Path {
            return None;
        }

        Some(self.inner.url.to_file_path().unwrap())
    }

    /// Returns `true` if this source is from a registry (either local or not).
    pub fn is_registry(self) -> bool {
        matches!(
            self.inner.kind,
            SourceKind::Registry | SourceKind::LocalRegistry
        )
    }

    /// Returns `true` if this source is a "remote" registry.
    ///
    /// "remote" may also mean a file URL to a git index, so it is not
    /// necessarily "remote". This just means it is not `local-registry`.
    pub fn is_remote_registry(self) -> bool {
        matches!(self.inner.kind, SourceKind::Registry)
    }

    /// Returns `true` if this source from a Git repository.
    pub fn is_git(self) -> bool {
        matches!(self.inner.kind, SourceKind::Git(_))
    }

    /// Creates an implementation of `Source` corresponding to this ID.
    pub fn load<'a>(
        self,
        config: &'a Config,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn super::Source + 'a>> {
        trace!("loading SourceId; {}", self);
        match self.inner.kind {
            SourceKind::Git(..) => Ok(Box::new(GitSource::new(self, config)?)),
            SourceKind::Path => {
                let path = match self.inner.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => panic!("path sources cannot be remote"),
                };
                Ok(Box::new(PathSource::new(&path, self, config)))
            }
            SourceKind::Registry => Ok(Box::new(RegistrySource::remote(
                self,
                yanked_whitelist,
                config,
            ))),
            SourceKind::LocalRegistry => {
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
            SourceKind::Directory => {
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
        self.inner.precise.as_deref()
    }

    /// Gets the Git reference if this is a git source, otherwise `None`.
    pub fn git_reference(self) -> Option<&'static GitReference> {
        match self.inner.kind {
            SourceKind::Git(ref s) => Some(s),
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
            SourceKind::Registry => {}
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

impl PartialEq for SourceId {
    fn eq(&self, other: &SourceId) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for SourceId {
    fn partial_cmp(&self, other: &SourceId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Custom comparison defined as canonical URL equality for git sources and URL
// equality for other sources, ignoring the `precise` and `name` fields.
impl Ord for SourceId {
    fn cmp(&self, other: &SourceId) -> Ordering {
        // If our interior pointers are to the exact same `SourceIdInner` then
        // we're guaranteed to be equal.
        if ptr::eq(self.inner, other.inner) {
            return Ordering::Equal;
        }

        // Sort first based on `kind`, deferring to the URL comparison below if
        // the kinds are equal.
        match self.inner.kind.cmp(&other.inner.kind) {
            Ordering::Equal => {}
            other => return other,
        }

        // If the `kind` and the `url` are equal, then for git sources we also
        // ensure that the canonical urls are equal.
        match (&self.inner.kind, &other.inner.kind) {
            (SourceKind::Git(_), SourceKind::Git(_)) => {
                self.inner.canonical_url.cmp(&other.inner.canonical_url)
            }
            _ => self.inner.url.cmp(&other.inner.url),
        }
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
            s.collect_str(&self.as_url())
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
            SourceKind::Git(ref reference) => {
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
            SourceKind::Path => write!(f, "{}", url_display(&self.inner.url)),
            SourceKind::Registry => write!(f, "registry `{}`", url_display(&self.inner.url)),
            SourceKind::LocalRegistry => write!(f, "registry `{}`", url_display(&self.inner.url)),
            SourceKind::Directory => write!(f, "dir {}", url_display(&self.inner.url)),
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
            SourceKind::Git(_) => self.inner.canonical_url.hash(into),
            _ => self.inner.url.as_str().hash(into),
        }
    }
}

// forward to `Ord`
impl PartialOrd for SourceKind {
    fn partial_cmp(&self, other: &SourceKind) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Note that this is specifically not derived on `SourceKind` although the
// implementation here is very similar to what it might look like if it were
// otherwise derived.
//
// The reason for this is somewhat obtuse. First of all the hash value of
// `SourceKind` makes its way into `~/.cargo/registry/index/github.com-XXXX`
// which means that changes to the hash means that all Rust users need to
// redownload the crates.io index and all their crates. If possible we strive to
// not change this to make this redownloading behavior happen as little as
// possible. How is this connected to `Ord` you might ask? That's a good
// question!
//
// Since the beginning of time `SourceKind` has had `#[derive(Hash)]`. It for
// the longest time *also* derived the `Ord` and `PartialOrd` traits. In #8522,
// however, the implementation of `Ord` changed. This handwritten implementation
// forgot to sync itself with the originally derived implementation, namely
// placing git dependencies as sorted after all other dependencies instead of
// first as before.
//
// This regression in #8522 (Rust 1.47) went unnoticed. When we switched back
// to a derived implementation in #9133 (Rust 1.52 beta) we only then ironically
// saw an issue (#9334). In #9334 it was observed that stable Rust at the time
// (1.51) was sorting git dependencies last, whereas Rust 1.52 beta would sort
// git dependencies first. This is because the `PartialOrd` implementation in
// 1.51 used #8522, the buggy implementation, which put git deps last. In 1.52
// it was (unknowingly) restored to the pre-1.47 behavior with git dependencies
// first.
//
// Because the breakage was only witnessed after the original breakage, this
// trait implementation is preserving the "broken" behavior. Put a different way:
//
// * Rust pre-1.47 sorted git deps first.
// * Rust 1.47 to Rust 1.51 sorted git deps last, a breaking change (#8522) that
//   was never noticed.
// * Rust 1.52 restored the pre-1.47 behavior (#9133, without knowing it did
//   so), and breakage was witnessed by actual users due to difference with
//   1.51.
// * Rust 1.52 (the source as it lives now) was fixed to match the 1.47-1.51
//   behavior (#9383), which is now considered intentionally breaking from the
//   pre-1.47 behavior.
//
// Note that this was all discovered when Rust 1.53 was in nightly and 1.52 was
// in beta. #9133 was in both beta and nightly at the time of discovery. For
// 1.52 #9383 reverted #9133, meaning 1.52 is the same as 1.51. On nightly
// (1.53) #9397 was created to fix the regression introduced by #9133 relative
// to the current stable (1.51).
//
// That's all a long winded way of saying "it's wierd that git deps hash first
// and are sorted last, but it's the way it is right now". The author of this
// comment chose to handwrite the `Ord` implementation instead of the `Hash`
// implementation, but it's only required that at most one of them is
// hand-written because the other can be derived. Perhaps one day in
// the future someone can figure out how to remove this behavior.
impl Ord for SourceKind {
    fn cmp(&self, other: &SourceKind) -> Ordering {
        match (self, other) {
            (SourceKind::Path, SourceKind::Path) => Ordering::Equal,
            (SourceKind::Path, _) => Ordering::Less,
            (_, SourceKind::Path) => Ordering::Greater,

            (SourceKind::Registry, SourceKind::Registry) => Ordering::Equal,
            (SourceKind::Registry, _) => Ordering::Less,
            (_, SourceKind::Registry) => Ordering::Greater,

            (SourceKind::LocalRegistry, SourceKind::LocalRegistry) => Ordering::Equal,
            (SourceKind::LocalRegistry, _) => Ordering::Less,
            (_, SourceKind::LocalRegistry) => Ordering::Greater,

            (SourceKind::Directory, SourceKind::Directory) => Ordering::Equal,
            (SourceKind::Directory, _) => Ordering::Less,
            (_, SourceKind::Directory) => Ordering::Greater,

            (SourceKind::Git(a), SourceKind::Git(b)) => a.cmp(b),
        }
    }
}

// This is a test that the hash of the `SourceId` for crates.io is a well-known
// value.
//
// Note that the hash value matches what the crates.io source id has hashed
// since long before Rust 1.30. We strive to keep this value the same across
// versions of Cargo because changing it means that users will need to
// redownload the index and all crates they use when using a new Cargo version.
//
// This isn't to say that this hash can *never* change, only that when changing
// this it should be explicitly done. If this hash changes accidentally and
// you're able to restore the hash to its original value, please do so!
// Otherwise please just leave a comment in your PR as to why the hash value is
// changing and why the old value can't be easily preserved.
#[test]
fn test_cratesio_hash() {
    let config = Config::default().unwrap();
    let crates_io = SourceId::crates_io(&config).unwrap();
    assert_eq!(crate::util::hex::short_hash(&crates_io), "1ecc6299db9ec823");
}

/// A `Display`able view into a `SourceId` that will write it as a url
pub struct SourceIdAsUrl<'a> {
    inner: &'a SourceIdInner,
}

impl<'a> fmt::Display for SourceIdAsUrl<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self.inner {
            SourceIdInner {
                kind: SourceKind::Path,
                ref url,
                ..
            } => write!(f, "path+{}", url),
            SourceIdInner {
                kind: SourceKind::Git(ref reference),
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
                kind: SourceKind::Registry,
                ref url,
                ..
            } => write!(f, "registry+{}", url),
            SourceIdInner {
                kind: SourceKind::LocalRegistry,
                ref url,
                ..
            } => write!(f, "local-registry+{}", url),
            SourceIdInner {
                kind: SourceKind::Directory,
                ref url,
                ..
            } => write!(f, "directory+{}", url),
        }
    }
}

impl GitReference {
    /// Returns a `Display`able view of this git reference, or None if using
    /// the head of the default branch
    pub fn pretty_ref(&self) -> Option<PrettyRef<'_>> {
        match self {
            GitReference::DefaultBranch => None,
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
            GitReference::DefaultBranch => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GitReference, SourceId, SourceKind};
    use crate::util::IntoUrl;

    #[test]
    fn github_sources_equal() {
        let loc = "https://github.com/foo/bar".into_url().unwrap();
        let default = SourceKind::Git(GitReference::DefaultBranch);
        let s1 = SourceId::new(default.clone(), loc).unwrap();

        let loc = "git://github.com/foo/bar".into_url().unwrap();
        let s2 = SourceId::new(default, loc.clone()).unwrap();

        assert_eq!(s1, s2);

        let foo = SourceKind::Git(GitReference::Branch("foo".to_string()));
        let s3 = SourceId::new(foo, loc).unwrap();
        assert_ne!(s1, s3);
    }
}
