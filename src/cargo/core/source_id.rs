use crate::core::GitReference;
use crate::core::PackageId;
use crate::core::SourceKind;
use crate::sources::registry::CRATES_IO_HTTP_INDEX;
use crate::sources::source::Source;
use crate::sources::{CRATES_IO_DOMAIN, CRATES_IO_INDEX, CRATES_IO_REGISTRY, DirectorySource};
use crate::sources::{GitSource, PathSource, RegistrySource};
use crate::util::interning::InternedString;
use crate::util::{CanonicalUrl, CargoResult, GlobalContext, IntoUrl, context};
use anyhow::Context as _;
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
                let reference = GitReference::from_query(url.query_pairs());
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

    /// Creates a `SourceId` from a filesystem path.
    ///
    /// `path`: an absolute path.
    pub fn for_manifest_path(manifest_path: &Path) -> CargoResult<SourceId> {
        if crate::util::toml::is_embedded(manifest_path) {
            Self::for_path(manifest_path)
        } else {
            Self::for_path(manifest_path.parent().unwrap())
        }
    }

    /// Creates a `SourceId` from a Git reference.
    pub fn for_git(url: &Url, reference: GitReference) -> CargoResult<SourceId> {
        SourceId::new(SourceKind::Git(reference), url.clone(), None)
    }

    /// Creates a `SourceId` from a remote registry URL when the registry name
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
    pub fn crates_io(gctx: &GlobalContext) -> CargoResult<SourceId> {
        gctx.crates_io_source_id()
    }

    /// Returns the `SourceId` corresponding to the main repository, using the
    /// sparse HTTP index if allowed.
    pub fn crates_io_maybe_sparse_http(gctx: &GlobalContext) -> CargoResult<SourceId> {
        if Self::crates_io_is_sparse(gctx)? {
            gctx.check_registry_index_not_set()?;
            let url = CRATES_IO_HTTP_INDEX.into_url().unwrap();
            let key = KeyOf::Registry(CRATES_IO_REGISTRY.into());
            SourceId::new(SourceKind::SparseRegistry, url, Some(key))
        } else {
            Self::crates_io(gctx)
        }
    }

    /// Returns whether to access crates.io over the sparse protocol.
    pub fn crates_io_is_sparse(gctx: &GlobalContext) -> CargoResult<bool> {
        let proto: Option<context::Value<String>> = gctx.get("registries.crates-io.protocol")?;
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
    pub fn alt_registry(gctx: &GlobalContext, key: &str) -> CargoResult<SourceId> {
        if key == CRATES_IO_REGISTRY {
            return Self::crates_io(gctx);
        }
        let url = gctx.get_registry_index(key)?;
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

    pub fn kind(&self) -> &SourceKind {
        &self.inner.kind
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
        gctx: &'a GlobalContext,
        yanked_whitelist: &HashSet<PackageId>,
    ) -> CargoResult<Box<dyn Source + 'a>> {
        trace!("loading SourceId; {}", self);
        match self.inner.kind {
            SourceKind::Git(..) => Ok(Box::new(GitSource::new(self, gctx)?)),
            SourceKind::Path => {
                let path = self
                    .inner
                    .url
                    .to_file_path()
                    .expect("path sources cannot be remote");
                if crate::util::toml::is_embedded(&path) {
                    anyhow::bail!("Single file packages cannot be used as dependencies")
                }
                Ok(Box::new(PathSource::new(&path, self, gctx)))
            }
            SourceKind::Registry | SourceKind::SparseRegistry => Ok(Box::new(
                RegistrySource::remote(self, yanked_whitelist, gctx)?,
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
                    gctx,
                )))
            }
            SourceKind::Directory => {
                let path = self
                    .inner
                    .url
                    .to_file_path()
                    .expect("path sources cannot be remote");
                Ok(Box::new(DirectorySource::new(&path, self, gctx)))
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
    /// from a call to [`SourceId::with_precise_registry_version`].
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
            Some(Precise::GitUrlFragment(s)) => Some(&s),
            _ => None,
        }
    }

    /// Creates a new `SourceId` from this source with the given `precise`.
    pub fn with_git_precise(self, fragment: Option<String>) -> SourceId {
        self.with_precise(&fragment.map(|f| Precise::GitUrlFragment(f)))
    }

    /// Creates a new `SourceId` from this source without a `precise`.
    pub fn without_precise(self) -> SourceId {
        self.with_precise(&None)
    }

    /// Creates a new `SourceId` from this source without a `precise`.
    pub fn with_locked_precise(self) -> SourceId {
        self.with_precise(&Some(Precise::Locked))
    }

    /// Creates a new `SourceId` from this source with the `precise` from some other `SourceId`.
    pub fn with_precise_from(self, v: Self) -> SourceId {
        self.with_precise(&v.inner.precise)
    }

    fn with_precise(self, precise: &Option<Precise>) -> SourceId {
        if &self.inner.precise == precise {
            self
        } else {
            SourceId::wrap(SourceIdInner {
                precise: precise.clone(),
                ..(*self.inner).clone()
            })
        }
    }

    /// When updating a lock file on a version using `cargo update --precise`
    /// the requested version is stored in the precise field.
    /// On a registry dependency we also need to keep track of the package that
    /// should be updated and even which of the versions should be updated.
    /// All of this gets encoded in the precise field using this method.
    /// The data can be read with [`SourceId::precise_registry_version`]
    pub fn with_precise_registry_version(
        self,
        name: InternedString,
        version: semver::Version,
        precise: &str,
    ) -> CargoResult<SourceId> {
        let precise = semver::Version::parse(precise).with_context(|| {
            if let Some(stripped) = precise.strip_prefix("v") {
                return format!(
                    "the version provided, `{precise}` is not a \
                    valid SemVer version\n\n\
                    help: try changing the version to `{stripped}`",
                );
            }
            format!("invalid version format for precise version `{precise}`")
        })?;

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

    /// Hashes `self` to be used in the name of some Cargo folders, so shouldn't vary.
    ///
    /// For git and url, `as_str` gives the serialisation of a url (which has a spec) and so
    /// insulates against possible changes in how the url crate does hashing.
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
        self.inner.kind.hash(into);
        match self.inner.kind {
            SourceKind::Git(_) => (&self).inner.canonical_url.hash(into),
            _ => (&self).inner.url.as_str().hash(into),
        }
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

// Custom comparison defined as source kind and canonical URL equality,
// ignoring the `precise` and `name` fields.
impl Ord for SourceId {
    fn cmp(&self, other: &SourceId) -> Ordering {
        // If our interior pointers are to the exact same `SourceIdInner` then
        // we're guaranteed to be equal.
        if ptr::eq(self.inner, other.inner) {
            return Ordering::Equal;
        }

        // Sort first based on `kind`, deferring to the URL comparison if
        // the kinds are equal.
        let ord_kind = self.inner.kind.cmp(&other.inner.kind);
        ord_kind.then_with(|| self.inner.canonical_url.cmp(&other.inner.canonical_url))
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
                if let Some(pretty) = reference.pretty_ref(true) {
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

impl Hash for SourceId {
    fn hash<S: hash::Hasher>(&self, into: &mut S) {
        self.inner.kind.hash(into);
        self.inner.canonical_url.hash(into);
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

/// A `Display`able view into a `SourceId` that will write it as a url
pub struct SourceIdAsUrl<'a> {
    inner: &'a SourceIdInner,
    encoded: bool,
}

impl<'a> fmt::Display for SourceIdAsUrl<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(protocol) = self.inner.kind.protocol() {
            write!(f, "{protocol}+")?;
        }
        write!(f, "{}", self.inner.url)?;
        if let SourceIdInner {
            kind: SourceKind::Git(ref reference),
            ref precise,
            ..
        } = *self.inner
        {
            if let Some(pretty) = reference.pretty_ref(self.encoded) {
                write!(f, "?{}", pretty)?;
            }
            if let Some(precise) = precise.as_ref() {
                write!(f, "#{}", precise)?;
            }
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
    use crate::util::{GlobalContext, IntoUrl};

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
    // since Rust 1.84.0. We strive to keep this value the same across
    // versions of Cargo because changing it means that users will need to
    // redownload the index and all crates they use when using a new Cargo version.
    //
    // This isn't to say that this hash can *never* change, only that when changing
    // this it should be explicitly done. If this hash changes accidentally and
    // you're able to restore the hash to its original value, please do so!
    // Otherwise please just leave a comment in your PR as to why the hash value is
    // changing and why the old value can't be easily preserved.
    // If it takes an ugly hack to restore it,
    // then leave a link here so we can remove the hack next time we change the hash.
    //
    // Hacks to remove next time the hash changes:
    // - (fill in your code here)
    //
    // The hash value should be stable across platforms, and doesn't depend on
    // endianness and bit-width. One caveat is that absolute paths on Windows
    // are inherently different than on Unix-like platforms. Unless we omit or
    // strip the prefix components (e.g. `C:`), there is not way to have a true
    // cross-platform stable hash for absolute paths.
    #[test]
    fn test_stable_hash() {
        use std::hash::Hasher;
        use std::path::Path;

        use snapbox::IntoData as _;
        use snapbox::assert_data_eq;
        use snapbox::str;

        use crate::util::StableHasher;
        use crate::util::hex::short_hash;

        #[cfg(not(windows))]
        let ws_root = Path::new("/tmp/ws");
        #[cfg(windows)]
        let ws_root = Path::new(r"C:\\tmp\ws");

        let gen_hash = |source_id: SourceId| {
            let mut hasher = StableHasher::new();
            source_id.stable_hash(ws_root, &mut hasher);
            Hasher::finish(&hasher).to_string()
        };

        let source_id = SourceId::crates_io(&GlobalContext::default().unwrap()).unwrap();
        assert_data_eq!(gen_hash(source_id), str!["7062945687441624357"].raw());
        assert_data_eq!(short_hash(&source_id), str!["25cdd57fae9f0462"].raw());

        let url = "https://my-crates.io".into_url().unwrap();
        let source_id = SourceId::for_registry(&url).unwrap();
        assert_data_eq!(gen_hash(source_id), str!["8310250053664888498"].raw());
        assert_data_eq!(short_hash(&source_id), str!["b2d65deb64f05373"].raw());

        let url = "https://your-crates.io".into_url().unwrap();
        let source_id = SourceId::for_alt_registry(&url, "alt").unwrap();
        assert_data_eq!(gen_hash(source_id), str!["14149534903000258933"].raw());
        assert_data_eq!(short_hash(&source_id), str!["755952de063f5dc4"].raw());

        let url = "sparse+https://my-crates.io".into_url().unwrap();
        let source_id = SourceId::for_registry(&url).unwrap();
        assert_data_eq!(gen_hash(source_id), str!["16249512552851930162"].raw());
        assert_data_eq!(short_hash(&source_id), str!["327cfdbd92dd81e1"].raw());

        let url = "sparse+https://your-crates.io".into_url().unwrap();
        let source_id = SourceId::for_alt_registry(&url, "alt").unwrap();
        assert_data_eq!(gen_hash(source_id), str!["6156697384053352292"].raw());
        assert_data_eq!(short_hash(&source_id), str!["64a713b6a6fb7055"].raw());

        let url = "file:///tmp/ws/crate".into_url().unwrap();
        let source_id = SourceId::for_git(&url, GitReference::DefaultBranch).unwrap();
        assert_data_eq!(gen_hash(source_id), str!["473480029881867801"].raw());
        assert_data_eq!(short_hash(&source_id), str!["199e591d94239206"].raw());

        let path = &ws_root.join("crate");
        let source_id = SourceId::for_local_registry(path).unwrap();
        #[cfg(not(windows))]
        {
            assert_data_eq!(gen_hash(source_id), str!["11515846423845066584"].raw());
            assert_data_eq!(short_hash(&source_id), str!["58d73c154f81d09f"].raw());
        }
        #[cfg(windows)]
        {
            assert_data_eq!(gen_hash(source_id), str!["6146331155906064276"].raw());
            assert_data_eq!(short_hash(&source_id), str!["946fb2239f274c55"].raw());
        }

        let source_id = SourceId::for_path(path).unwrap();
        assert_data_eq!(gen_hash(source_id), str!["215644081443634269"].raw());
        #[cfg(not(windows))]
        assert_data_eq!(short_hash(&source_id), str!["64bace89c92b101f"].raw());
        #[cfg(windows)]
        assert_data_eq!(short_hash(&source_id), str!["01e1e6c391813fb6"].raw());

        let source_id = SourceId::for_directory(path).unwrap();
        #[cfg(not(windows))]
        {
            assert_data_eq!(gen_hash(source_id), str!["6127590343904940368"].raw());
            assert_data_eq!(short_hash(&source_id), str!["505191d1f3920955"].raw());
        }
        #[cfg(windows)]
        {
            assert_data_eq!(gen_hash(source_id), str!["10423446877655960172"].raw());
            assert_data_eq!(short_hash(&source_id), str!["6c8ad69db585a790"].raw());
        }
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
