use crate::core::PackageId;
use crate::sources::registry::CRATES_IO_HTTP_INDEX;
use crate::sources::source::Source;
use crate::sources::{DirectorySource, CRATES_IO_DOMAIN, CRATES_IO_INDEX, CRATES_IO_REGISTRY};
use crate::sources::{GitSource, PathSource, RegistrySource};
use crate::util::interning::InternedString;
use crate::util::{config, CanonicalUrl, CargoResult, Config, IntoUrl};
use anyhow::Context;
use serde::de;
use serde::ser;
use std::cmp::{self, Ordering};
use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::hash::{self, Hash};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::trace;
use url::Url;

static SOURCE_ID_CACHE: OnceLock<Mutex<HashSet<&'static SourceIdInner>>> = OnceLock::new();

/// Unique identifier for a source of packages.
///
/// Cargo uniquely identifies packages using [`PackageId`], a combination of the
/// package name, version, and the code source. `SourceId` exactly represents
/// the "code source" in `PackageId`. See [`SourceId::hash`] to learn what are
/// taken into account for the uniqueness of a source.
///
/// `SourceId` is usually associated with an instance of [`Source`], which is
/// supposed to provide a `SourceId` via [`Source::source_id`] method.
///
/// [`Source`]: crate::sources::source::Source
/// [`Source::source_id`]: crate::sources::source::Source::source_id
/// [`PackageId`]: super::PackageId
#[derive(Clone, Copy, Eq, Debug)]
pub struct SourceId {
    inner: &'static SourceIdInner,
}

/// The interned version of [`SourceId`] to avoid excessive clones and borrows.
/// Values are cached in `SOURCE_ID_CACHE` once created.
#[derive(Eq, Clone, Debug)]
struct SourceIdInner {
    /// The source URL.
    url: Url,
    /// The canonical version of the above url. See [`CanonicalUrl`] to learn
    /// why it is needed and how it normalizes a URL.
    canonical_url: CanonicalUrl,
    /// The source kind.
    kind: SourceKind,
    /// For example, the exact Git revision of the specified branch for a Git Source.
    precise: Option<Precise>,
    /// Name of the remote registry.
    ///
    /// WARNING: this is not always set when the name is not known,
    /// e.g. registry coming from `--index` or Cargo.lock
    registry_key: Option<KeyOf>,
}

#[derive(Eq, PartialEq, Clone, Debug, Hash)]
enum Precise {
    Locked,
    Updated {
        name: InternedString,
        from: semver::Version,
        to: semver::Version,
    },
    GitUrlFragment(String),
}

impl fmt::Display for Precise {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Precise::Locked => "locked".fmt(f),
            Precise::Updated { name, from, to } => {
                write!(f, "{name}={from}->{to}")
            }
            Precise::GitUrlFragment(s) => s.fmt(f),
        }
    }
}

/// The possible kinds of code source.
/// Along with [`SourceIdInner`], this fully defines the source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SourceKind {
    /// A git repository.
    Git(GitReference),
    /// A local path.
    Path,
    /// A remote registry.
    Registry,
    /// A sparse registry.
    SparseRegistry,
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
    /// From a specific revision. Can be a commit hash (either short or full),
    /// or a named reference like `refs/pull/493/head`.
    Rev(String),
    /// The default branch of the repository, the reference named `HEAD`.
    DefaultBranch,
}

/// Where the remote source key is defined.
///
/// The purpose of this is to provide better diagnostics for different sources of keys.
#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyOf {
    /// Defined in the `[registries]` table or the built-in `crates-io` key.
    Registry(String),
    /// Defined in the `[source]` replacement table.
    Source(String),
}

impl SourceId {
    /// Creates a `SourceId` object from the kind and URL.
    ///
    /// The canonical url will be calculated, but the precise field will not
    fn new(kind: SourceKind, url: Url, key: Option<KeyOf>) -> CargoResult<SourceId> {
        if kind == SourceKind::SparseRegistry {
            // Sparse URLs are different because they store the kind prefix (sparse+)
            // in the URL. This is because the prefix is necessary to differentiate
            // from regular registries (git-based). The sparse+ prefix is included
            // everywhere, including user-facing locations such as the `config.toml`
            // file that defines the registry, or whenever Cargo displays it to the user.
            assert!(url.as_str().starts_with("sparse+"));
        }
        let source_id = SourceId::wrap(SourceIdInner {
            kind,
            canonical_url: CanonicalUrl::new(&url)?,
            url,
            precise: None,
            registry_key: key,
        });
        Ok(source_id)
    }

    /// Interns the value and returns the wrapped type.
    fn wrap(inner: SourceIdInner) -> SourceId {
        let mut cache = SOURCE_ID_CACHE
            .get_or_init(|| Default::default())
            .lock()
            .unwrap();
        let inner = cache.get(&inner).cloned().unwrap_or_else(|| {
            let inner = Box::leak(Box::new(inner));
            cache.insert(inner);
            inner
        });
        SourceId { inner }
    }

    fn remote_source_kind(url: &Url) -> SourceKind {
        if url.as_str().starts_with("sparse+") {
            SourceKind::SparseRegistry
        } else {
            SourceKind::Registry
        }
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
        let (kind, url) = string
            .split_once('+')
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
                Ok(SourceId::for_git(&url, reference)?.with_git_precise(precise))
            }
            "registry" => {
                let url = url.into_url()?;
                Ok(SourceId::new(SourceKind::Registry, url, None)?.with_locked_precise())
            }
            "sparse" => {
                let url = string.into_url()?;
                Ok(SourceId::new(SourceKind::SparseRegistry, url, None)?.with_locked_precise())
            }
            "path" => {
                let url = url.into_url()?;
                SourceId::new(SourceKind::Path, url, None)
            }
            kind => Err(anyhow::format_err!("unsupported source protocol: {}", kind)),
        }
    }

    /// A view of the [`SourceId`] that can be `Display`ed as a URL.
    pub fn as_url(&self) -> SourceIdAsUrl<'_> {
        SourceIdAsUrl {
            inner: &*self.inner,
            encoded: false,
        }
    }

    /// Like [`Self::as_url`] but with URL parameters encoded.
    pub fn as_encoded_url(&self) -> SourceIdAsUrl<'_> {
        SourceIdAsUrl {
            inner: &*self.inner,
            encoded: true,
        }
    }

    /// Creates a `SourceId` from a filesystem path.
    ///
    /// `path`: an absolute path.
    pub fn for_path(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(SourceKind::Path, url, None)
    }

    /// Creates a `SourceId` from a Git reference.
    pub fn for_git(url: &Url, reference: GitReference) -> CargoResult<SourceId> {
        SourceId::new(SourceKind::Git(reference), url.clone(), None)
    }

    /// Creates a SourceId from a remote registry URL when the registry name
    /// cannot be determined, e.g. a user passes `--index` directly from CLI.
    ///
    /// Use [`SourceId::for_alt_registry`] if a name can provided, which
    /// generates better messages for cargo.
    pub fn for_registry(url: &Url) -> CargoResult<SourceId> {
        let kind = Self::remote_source_kind(url);
        SourceId::new(kind, url.to_owned(), None)
    }

    /// Creates a `SourceId` for a remote registry from the `[registries]` table or crates.io.
    pub fn for_alt_registry(url: &Url, key: &str) -> CargoResult<SourceId> {
        let kind = Self::remote_source_kind(url);
        let key = KeyOf::Registry(key.into());
        SourceId::new(kind, url.to_owned(), Some(key))
    }

    /// Creates a `SourceId` for a remote registry from the `[source]` replacement table.
    pub fn for_source_replacement_registry(url: &Url, key: &str) -> CargoResult<SourceId> {
        let kind = Self::remote_source_kind(url);
        let key = KeyOf::Source(key.into());
        SourceId::new(kind, url.to_owned(), Some(key))
    }

    /// Creates a `SourceId` from a local registry path.
    pub fn for_local_registry(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(SourceKind::LocalRegistry, url, None)
    }

    /// Creates a `SourceId` from a directory path.
    pub fn for_directory(path: &Path) -> CargoResult<SourceId> {
        let url = path.into_url()?;
        SourceId::new(SourceKind::Directory, url, None)
    }

    /// Returns the `SourceId` corresponding to the main repository.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a `.cargo/config.toml`.
    pub fn crates_io(config: &Config) -> CargoResult<SourceId> {
        config.crates_io_source_id()
    }

    /// Returns the `SourceId` corresponding to the main repository, using the
    /// sparse HTTP index if allowed.
    pub fn crates_io_maybe_sparse_http(config: &Config) -> CargoResult<SourceId> {
        if Self::crates_io_is_sparse(config)? {
            config.check_registry_index_not_set()?;
            let url = CRATES_IO_HTTP_INDEX.into_url().unwrap();
            let key = KeyOf::Registry(CRATES_IO_REGISTRY.into());
            SourceId::new(SourceKind::SparseRegistry, url, Some(key))
        } else {
            Self::crates_io(config)
        }
    }

    /// Returns whether to access crates.io over the sparse protocol.
    pub fn crates_io_is_sparse(config: &Config) -> CargoResult<bool> {
        let proto: Option<config::Value<String>> = config.get("registries.crates-io.protocol")?;
        let is_sparse = match proto.as_ref().map(|v| v.val.as_str()) {
            Some("sparse") => true,
            Some("git") => false,
            Some(unknown) => anyhow::bail!(
                "unsupported registry protocol `{unknown}` (defined in {})",
                proto.as_ref().unwrap().definition
            ),
            None => true,
        };
        Ok(is_sparse)
    }

    /// Gets the `SourceId` associated with given name of the remote registry.
    pub fn alt_registry(config: &Config, key: &str) -> CargoResult<SourceId> {
        if key == CRATES_IO_REGISTRY {
            return Self::crates_io(config);
        }
        let url = config.get_registry_index(key)?;
        Self::for_alt_registry(&url, key)
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

    /// Displays the text "crates.io index" for Cargo shell status output.
    pub fn display_index(self) -> String {
        if self.is_crates_io() {
            format!("{} index", CRATES_IO_DOMAIN)
        } else {
            format!("`{}` index", self.display_registry_name())
        }
    }

    /// Displays the name of a registry if it has one. Otherwise just the URL.
    pub fn display_registry_name(self) -> String {
        if let Some(key) = self.inner.registry_key.as_ref().map(|k| k.key()) {
            key.into()
        } else if self.has_precise() {
            // We remove `precise` here to retrieve an permissive version of
            // `SourceIdInner`, which may contain the registry name.
            self.without_precise().display_registry_name()
        } else {
            url_display(self.url())
        }
    }

    /// Gets the name of the remote registry as defined in the `[registries]` table,
    /// or the built-in `crates-io` key.
    pub fn alt_registry_key(&self) -> Option<&str> {
        self.inner.registry_key.as_ref()?.alternative_registry()
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
            SourceKind::Registry | SourceKind::SparseRegistry | SourceKind::LocalRegistry
        )
    }

    /// Returns `true` if this source is from a sparse registry.
    pub fn is_sparse(self) -> bool {
        matches!(self.inner.kind, SourceKind::SparseRegistry)
    }

    /// Returns `true` if this source is a "remote" registry.
    ///
    /// "remote" may also mean a file URL to a git index, so it is not
    /// necessarily "remote". This just means it is not `local-registry`.
    pub fn is_remote_registry(self) -> bool {
        matches!(
            self.inner.kind,
            SourceKind::Registry | SourceKind::SparseRegistry
        )
    }

    /// Returns `true` if this source from a Git repository.
    pub fn is_git(self) -> bool {
        matches!(self.inner.kind, SourceKind::Git(_))
    }

    /// Creates an implementation of `Source` corresponding to this ID.
    ///
    /// * `yanked_whitelist` --- Packages allowed to be used, even if they are yanked.
    pub fn load<'a>(
        self,
        config: &'a Config,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn Source + 'a>> {
        trace!("loading SourceId; {}", self);
        match self.inner.kind {
            SourceKind::Git(..) => Ok(Box::new(GitSource::new(self, config)?)),
            SourceKind::Path => {
                let path = self
                    .inner
                    .url
                    .to_file_path()
                    .expect("path sources cannot be remote");
                Ok(Box::new(PathSource::new(&path, self, config)))
            }
            SourceKind::Registry | SourceKind::SparseRegistry => Ok(Box::new(
                RegistrySource::remote(self, yanked_whitelist, config)?,
            )),
            SourceKind::LocalRegistry => {
                let path = self
                    .inner
                    .url
                    .to_file_path()
                    .expect("path sources cannot be remote");
                Ok(Box::new(RegistrySource::local(
                    self,
                    &path,
                    yanked_whitelist,
                    config,
                )))
            }
            SourceKind::Directory => {
                let path = self
                    .inner
                    .url
                    .to_file_path()
                    .expect("path sources cannot be remote");
                Ok(Box::new(DirectorySource::new(&path, self, config)))
            }
        }
    }

    /// Gets the Git reference if this is a git source, otherwise `None`.
    pub fn git_reference(self) -> Option<&'static GitReference> {
        match self.inner.kind {
            SourceKind::Git(ref s) => Some(s),
            _ => None,
        }
    }

    /// Check if the precise data field has bean set
    pub fn has_precise(self) -> bool {
        self.inner.precise.is_some()
    }

    /// Check if the precise data field has bean set to "locked"
    pub fn has_locked_precise(self) -> bool {
        self.inner.precise == Some(Precise::Locked)
    }

    /// Check if two sources have the same precise data field
    pub fn has_same_precise_as(self, other: Self) -> bool {
        self.inner.precise == other.inner.precise
    }

    /// Check if the precise data field stores information for this `name`
    /// from a call to [SourceId::with_precise_registry_version].
    ///
    /// If so return the version currently in the lock file and the version to be updated to.
    pub fn precise_registry_version(
        self,
        pkg: &str,
    ) -> Option<(&semver::Version, &semver::Version)> {
        match &self.inner.precise {
            Some(Precise::Updated { name, from, to }) if name == pkg => Some((from, to)),
            _ => None,
        }
    }

    pub fn precise_git_fragment(self) -> Option<&'static str> {
        match &self.inner.precise {
            Some(Precise::GitUrlFragment(s)) => Some(&s[..8]),
            _ => None,
        }
    }

    pub fn precise_git_oid(self) -> CargoResult<Option<git2::Oid>> {
        Ok(match self.inner.precise.as_ref() {
            Some(Precise::GitUrlFragment(s)) => {
                Some(git2::Oid::from_str(s).with_context(|| {
                    format!("precise value for git is not a git revision: {}", s)
                })?)
            }
            _ => None,
        })
    }

    /// Creates a new `SourceId` from this source with the given `precise`.
    pub fn with_git_precise(self, fragment: Option<String>) -> SourceId {
        SourceId::wrap(SourceIdInner {
            precise: fragment.map(|f| Precise::GitUrlFragment(f)),
            ..(*self.inner).clone()
        })
    }

    /// Creates a new `SourceId` from this source without a `precise`.
    pub fn without_precise(self) -> SourceId {
        SourceId::wrap(SourceIdInner {
            precise: None,
            ..(*self.inner).clone()
        })
    }

    /// Creates a new `SourceId` from this source without a `precise`.
    pub fn with_locked_precise(self) -> SourceId {
        SourceId::wrap(SourceIdInner {
            precise: Some(Precise::Locked),
            ..(*self.inner).clone()
        })
    }

    /// Creates a new `SourceId` from this source with the `precise` from some other `SourceId`.
    pub fn with_precise_from(self, v: Self) -> SourceId {
        SourceId::wrap(SourceIdInner {
            precise: v.inner.precise.clone(),
            ..(*self.inner).clone()
        })
    }

    /// When updating a lock file on a version using `cargo update --precise`
    /// the requested version is stored in the precise field.
    /// On a registry dependency we also need to keep track of the package that
    /// should be updated and even which of the versions should be updated.
    /// All of this gets encoded in the precise field using this method.
    /// The data can be read with [SourceId::precise_registry_version]
    pub fn with_precise_registry_version(
        self,
        name: InternedString,
        version: semver::Version,
        precise: &str,
    ) -> CargoResult<SourceId> {
        let precise = semver::Version::parse(precise)
            .with_context(|| format!("invalid version format for precise version `{precise}`"))?;

        Ok(SourceId::wrap(SourceIdInner {
            precise: Some(Precise::Updated {
                name,
                from: version,
                to: precise,
            }),
            ..(*self.inner).clone()
        }))
    }

    /// Returns `true` if the remote registry is the standard <https://crates.io>.
    pub fn is_crates_io(self) -> bool {
        match self.inner.kind {
            SourceKind::Registry | SourceKind::SparseRegistry => {}
            _ => return false,
        }
        let url = self.inner.url.as_str();
        url == CRATES_IO_INDEX || url == CRATES_IO_HTTP_INDEX || is_overridden_crates_io_url(url)
    }

    /// Hashes `self`.
    ///
    /// For paths, remove the workspace prefix so the same source will give the
    /// same hash in different locations, helping reproducible builds.
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
                // TODO(-Znext-lockfile-bump): set it to true when stabilizing
                // lockfile v4, because we want Source ID serialization to be
                // consistent with lockfile.
                if let Some(pretty) = reference.pretty_ref(false) {
                    write!(f, "?{}", pretty)?;
                }

                if let Some(s) = &self.inner.precise {
                    let s = s.to_string();
                    let len = cmp::min(s.len(), 8);
                    write!(f, "#{}", &s[..len])?;
                }
                Ok(())
            }
            SourceKind::Path => write!(f, "{}", url_display(&self.inner.url)),
            SourceKind::Registry | SourceKind::SparseRegistry => {
                write!(f, "registry `{}`", self.display_registry_name())
            }
            SourceKind::LocalRegistry => write!(f, "registry `{}`", url_display(&self.inner.url)),
            SourceKind::Directory => write!(f, "dir {}", url_display(&self.inner.url)),
        }
    }
}

/// The hash of SourceId is used in the name of some Cargo folders, so shouldn't
/// vary. `as_str` gives the serialisation of a url (which has a spec) and so
/// insulates against possible changes in how the url crate does hashing.
impl Hash for SourceId {
    fn hash<S: hash::Hasher>(&self, into: &mut S) {
        self.inner.kind.hash(into);
        match self.inner.kind {
            SourceKind::Git(_) => self.inner.canonical_url.hash(into),
            _ => self.inner.url.as_str().hash(into),
        }
    }
}

/// The hash of `SourceIdInner` is used to retrieve its interned value from
/// `SOURCE_ID_CACHE`. We only care about fields that make `SourceIdInner`
/// unique. Optional fields not affecting the uniqueness must be excluded,
/// such as [`registry_key`]. That's why this is not derived.
///
/// [`registry_key`]: SourceIdInner::registry_key
impl Hash for SourceIdInner {
    fn hash<S: hash::Hasher>(&self, into: &mut S) {
        self.kind.hash(into);
        self.precise.hash(into);
        self.canonical_url.hash(into);
    }
}

/// This implementation must be synced with [`SourceIdInner::hash`].
impl PartialEq for SourceIdInner {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.precise == other.precise
            && self.canonical_url == other.canonical_url
    }
}

/// Forwards to `Ord`
impl PartialOrd for SourceKind {
    fn partial_cmp(&self, other: &SourceKind) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Note that this is specifically not derived on `SourceKind` although the
/// implementation here is very similar to what it might look like if it were
/// otherwise derived.
///
/// The reason for this is somewhat obtuse. First of all the hash value of
/// `SourceKind` makes its way into `~/.cargo/registry/index/github.com-XXXX`
/// which means that changes to the hash means that all Rust users need to
/// redownload the crates.io index and all their crates. If possible we strive
/// to not change this to make this redownloading behavior happen as little as
/// possible. How is this connected to `Ord` you might ask? That's a good
/// question!
///
/// Since the beginning of time `SourceKind` has had `#[derive(Hash)]`. It for
/// the longest time *also* derived the `Ord` and `PartialOrd` traits. In #8522,
/// however, the implementation of `Ord` changed. This handwritten implementation
/// forgot to sync itself with the originally derived implementation, namely
/// placing git dependencies as sorted after all other dependencies instead of
/// first as before.
///
/// This regression in #8522 (Rust 1.47) went unnoticed. When we switched back
/// to a derived implementation in #9133 (Rust 1.52 beta) we only then ironically
/// saw an issue (#9334). In #9334 it was observed that stable Rust at the time
/// (1.51) was sorting git dependencies last, whereas Rust 1.52 beta would sort
/// git dependencies first. This is because the `PartialOrd` implementation in
/// 1.51 used #8522, the buggy implementation, which put git deps last. In 1.52
/// it was (unknowingly) restored to the pre-1.47 behavior with git dependencies
/// first.
///
/// Because the breakage was only witnessed after the original breakage, this
/// trait implementation is preserving the "broken" behavior. Put a different way:
///
/// * Rust pre-1.47 sorted git deps first.
/// * Rust 1.47 to Rust 1.51 sorted git deps last, a breaking change (#8522) that
///   was never noticed.
/// * Rust 1.52 restored the pre-1.47 behavior (#9133, without knowing it did
///   so), and breakage was witnessed by actual users due to difference with
///   1.51.
/// * Rust 1.52 (the source as it lives now) was fixed to match the 1.47-1.51
///   behavior (#9383), which is now considered intentionally breaking from the
///   pre-1.47 behavior.
///
/// Note that this was all discovered when Rust 1.53 was in nightly and 1.52 was
/// in beta. #9133 was in both beta and nightly at the time of discovery. For
/// 1.52 #9383 reverted #9133, meaning 1.52 is the same as 1.51. On nightly
/// (1.53) #9397 was created to fix the regression introduced by #9133 relative
/// to the current stable (1.51).
///
/// That's all a long winded way of saying "it's weird that git deps hash first
/// and are sorted last, but it's the way it is right now". The author of this
/// comment chose to handwrite the `Ord` implementation instead of the `Hash`
/// implementation, but it's only required that at most one of them is
/// hand-written because the other can be derived. Perhaps one day in
/// the future someone can figure out how to remove this behavior.
impl Ord for SourceKind {
    fn cmp(&self, other: &SourceKind) -> Ordering {
        match (self, other) {
            (SourceKind::Path, SourceKind::Path) => Ordering::Equal,
            (SourceKind::Path, _) => Ordering::Less,
            (_, SourceKind::Path) => Ordering::Greater,

            (SourceKind::Registry, SourceKind::Registry) => Ordering::Equal,
            (SourceKind::Registry, _) => Ordering::Less,
            (_, SourceKind::Registry) => Ordering::Greater,

            (SourceKind::SparseRegistry, SourceKind::SparseRegistry) => Ordering::Equal,
            (SourceKind::SparseRegistry, _) => Ordering::Less,
            (_, SourceKind::SparseRegistry) => Ordering::Greater,

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

/// A `Display`able view into a `SourceId` that will write it as a url
pub struct SourceIdAsUrl<'a> {
    inner: &'a SourceIdInner,
    encoded: bool,
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
                if let Some(pretty) = reference.pretty_ref(self.encoded) {
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
            } => {
                write!(f, "registry+{url}")
            }
            SourceIdInner {
                kind: SourceKind::SparseRegistry,
                ref url,
                ..
            } => {
                // Sparse registry URL already includes the `sparse+` prefix
                write!(f, "{url}")
            }
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
    pub fn pretty_ref(&self, url_encoded: bool) -> Option<PrettyRef<'_>> {
        match self {
            GitReference::DefaultBranch => None,
            _ => Some(PrettyRef {
                inner: self,
                url_encoded,
            }),
        }
    }
}

/// A git reference that can be `Display`ed
pub struct PrettyRef<'a> {
    inner: &'a GitReference,
    url_encoded: bool,
}

impl<'a> fmt::Display for PrettyRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value: &str;
        match self.inner {
            GitReference::Branch(s) => {
                write!(f, "branch=")?;
                value = s;
            }
            GitReference::Tag(s) => {
                write!(f, "tag=")?;
                value = s;
            }
            GitReference::Rev(s) => {
                write!(f, "rev=")?;
                value = s;
            }
            GitReference::DefaultBranch => unreachable!(),
        }
        if self.url_encoded {
            for value in url::form_urlencoded::byte_serialize(value.as_bytes()) {
                write!(f, "{value}")?;
            }
        } else {
            write!(f, "{value}")?;
        }
        Ok(())
    }
}

impl KeyOf {
    /// Gets the underlying key.
    fn key(&self) -> &str {
        match self {
            KeyOf::Registry(k) | KeyOf::Source(k) => k,
        }
    }

    /// Gets the key if it's from an alternative registry.
    fn alternative_registry(&self) -> Option<&str> {
        match self {
            KeyOf::Registry(k) => Some(k),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GitReference, SourceId, SourceKind};
    use crate::util::{Config, IntoUrl};

    #[test]
    fn github_sources_equal() {
        let loc = "https://github.com/foo/bar".into_url().unwrap();
        let default = SourceKind::Git(GitReference::DefaultBranch);
        let s1 = SourceId::new(default.clone(), loc, None).unwrap();

        let loc = "git://github.com/foo/bar".into_url().unwrap();
        let s2 = SourceId::new(default, loc.clone(), None).unwrap();

        assert_eq!(s1, s2);

        let foo = SourceKind::Git(GitReference::Branch("foo".to_string()));
        let s3 = SourceId::new(foo, loc, None).unwrap();
        assert_ne!(s1, s3);
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
    //
    // The hash value depends on endianness and bit-width, so we only run this test on
    // little-endian 64-bit CPUs (such as x86-64 and ARM64) where it matches the
    // well-known value.
    #[test]
    #[cfg(all(target_endian = "little", target_pointer_width = "64"))]
    fn test_cratesio_hash() {
        let config = Config::default().unwrap();
        let crates_io = SourceId::crates_io(&config).unwrap();
        assert_eq!(crate::util::hex::short_hash(&crates_io), "1ecc6299db9ec823");
    }

    // See the comment in `test_cratesio_hash`.
    //
    // Only test on non-Windows as paths on Windows will get different hashes.
    #[test]
    #[cfg(all(target_endian = "little", target_pointer_width = "64", not(windows)))]
    fn test_stable_hash() {
        use std::hash::Hasher;
        use std::path::Path;

        let gen_hash = |source_id: SourceId| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            source_id.stable_hash(Path::new("/tmp/ws"), &mut hasher);
            hasher.finish()
        };

        let url = "https://my-crates.io".into_url().unwrap();
        let source_id = SourceId::for_registry(&url).unwrap();
        assert_eq!(gen_hash(source_id), 18108075011063494626);
        assert_eq!(crate::util::hex::short_hash(&source_id), "fb60813d6cb8df79");

        let url = "https://your-crates.io".into_url().unwrap();
        let source_id = SourceId::for_alt_registry(&url, "alt").unwrap();
        assert_eq!(gen_hash(source_id), 12862859764592646184);
        assert_eq!(crate::util::hex::short_hash(&source_id), "09c10fd0cbd74bce");

        let url = "sparse+https://my-crates.io".into_url().unwrap();
        let source_id = SourceId::for_registry(&url).unwrap();
        assert_eq!(gen_hash(source_id), 8763561830438022424);
        assert_eq!(crate::util::hex::short_hash(&source_id), "d1ea0d96f6f759b5");

        let url = "sparse+https://your-crates.io".into_url().unwrap();
        let source_id = SourceId::for_alt_registry(&url, "alt").unwrap();
        assert_eq!(gen_hash(source_id), 5159702466575482972);
        assert_eq!(crate::util::hex::short_hash(&source_id), "135d23074253cb78");

        let url = "file:///tmp/ws/crate".into_url().unwrap();
        let source_id = SourceId::for_git(&url, GitReference::DefaultBranch).unwrap();
        assert_eq!(gen_hash(source_id), 15332537265078583985);
        assert_eq!(crate::util::hex::short_hash(&source_id), "73a808694abda756");

        let path = Path::new("/tmp/ws/crate");

        let source_id = SourceId::for_local_registry(path).unwrap();
        assert_eq!(gen_hash(source_id), 18446533307730842837);
        assert_eq!(crate::util::hex::short_hash(&source_id), "52a84cc73f6fd48b");

        let source_id = SourceId::for_path(path).unwrap();
        assert_eq!(gen_hash(source_id), 8764714075439899829);
        assert_eq!(crate::util::hex::short_hash(&source_id), "e1ddd48578620fc1");

        let source_id = SourceId::for_directory(path).unwrap();
        assert_eq!(gen_hash(source_id), 17459999773908528552);
        assert_eq!(crate::util::hex::short_hash(&source_id), "6568fe2c2fab5bfe");
    }

    #[test]
    fn serde_roundtrip() {
        let url = "sparse+https://my-crates.io/".into_url().unwrap();
        let source_id = SourceId::for_registry(&url).unwrap();
        let formatted = format!("{}", source_id.as_url());
        let deserialized = SourceId::from_url(&formatted).unwrap();
        assert_eq!(formatted, "sparse+https://my-crates.io/");
        assert_eq!(source_id, deserialized);
    }

    #[test]
    fn gitrefs_roundtrip() {
        let base = "https://host/path".into_url().unwrap();
        let branch = GitReference::Branch("*-._+20%30 Z/z#foo=bar&zap[]?to\\()'\"".to_string());
        let s1 = SourceId::for_git(&base, branch).unwrap();
        let ser1 = format!("{}", s1.as_encoded_url());
        let s2 = SourceId::from_url(&ser1).expect("Failed to deserialize");
        let ser2 = format!("{}", s2.as_encoded_url());
        // Serializing twice should yield the same result
        assert_eq!(ser1, ser2, "Serialized forms don't match");
        // SourceId serializing the same should have the same semantics
        // This used to not be the case (# was ambiguous)
        assert_eq!(s1, s2, "SourceId doesn't round-trip");
        // Freeze the format to match an x-www-form-urlencoded query string
        // https://url.spec.whatwg.org/#application/x-www-form-urlencoded
        assert_eq!(
            ser1,
            "git+https://host/path?branch=*-._%2B20%2530+Z%2Fz%23foo%3Dbar%26zap%5B%5D%3Fto%5C%28%29%27%22"
        );
    }
}

/// Check if `url` equals to the overridden crates.io URL.
// ALLOWED: For testing Cargo itself only.
#[allow(clippy::disallowed_methods)]
fn is_overridden_crates_io_url(url: &str) -> bool {
    std::env::var("__CARGO_TEST_CRATES_IO_URL_DO_NOT_USE_THIS").map_or(false, |v| v == url)
}
