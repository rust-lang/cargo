use std::cmp::{self, Ordering};
use std::collections::hash_map::{HashMap, Values, IterMut};
use std::fmt::{self, Formatter};
use std::hash;
use std::mem;
use std::path::Path;
use std::sync::Arc;
use rustc_serialize::{Decodable, Decoder, Encodable, Encoder};

use url::Url;

use core::{Summary, Package, PackageId, Registry, Dependency};
use sources::{PathSource, GitSource, RegistrySource};
use sources::git;
use util::{human, Config, CargoResult, ToUrl};

/// A Source finds and downloads remote packages based on names and
/// versions.
pub trait Source: Registry {
    /// The update method performs any network operations required to
    /// get the entire list of all names, versions and dependencies of
    /// packages managed by the Source.
    fn update(&mut self) -> CargoResult<()>;

    /// The download method fetches the full package for each name and
    /// version specified.
    fn download(&mut self, packages: &[PackageId]) -> CargoResult<()>;

    /// The get method returns the Path of each specified package on the
    /// local file system. It assumes that `download` was already called,
    /// and that the packages are already locally available on the file
    /// system.
    fn get(&self, packages: &[PackageId]) -> CargoResult<Vec<Package>>;

    /// Generates a unique string which represents the fingerprint of the
    /// current state of the source.
    ///
    /// This fingerprint is used to determine the "fresheness" of the source
    /// later on. It must be guaranteed that the fingerprint of a source is
    /// constant if and only if the output product will remain constant.
    ///
    /// The `pkg` argument is the package which this fingerprint should only be
    /// interested in for when this source may contain multiple packages.
    fn fingerprint(&self, pkg: &Package) -> CargoResult<String>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    /// Kind::Git(<git reference>) represents a git repository
    Git(GitReference),
    /// represents a local path
    Path,
    /// represents the central registry
    Registry,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitReference {
    Tag(String),
    Branch(String),
    Rev(String),
}

/// Unique identifier for a source of packages.
#[derive(Clone, Eq, Debug)]
pub struct SourceId {
    inner: Arc<SourceIdInner>,
}

#[derive(Eq, Clone, Debug)]
struct SourceIdInner {
    url: Url,
    canonical_url: Url,
    kind: Kind,
    // e.g. the exact git revision of the specified branch for a Git Source
    precise: Option<String>
}

impl SourceId {
    fn new(kind: Kind, url: Url) -> SourceId {
        SourceId {
            inner: Arc::new(SourceIdInner {
                kind: kind,
                canonical_url: git::canonicalize_url(&url),
                url: url,
                precise: None,
            }),
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
    ///                     656c58fb7c5ef5f12bc747f".to_string());
    /// ```
    pub fn from_url(string: String) -> SourceId {
        let mut parts = string.splitn(2, '+');
        let kind = parts.next().unwrap();
        let url = parts.next().unwrap();

        match kind {
            "git" => {
                let mut url = url.to_url().unwrap();
                let mut reference = GitReference::Branch("master".to_string());
                let pairs = url.query_pairs().unwrap_or(Vec::new());
                for &(ref k, ref v) in pairs.iter() {
                    match &k[..] {
                        // map older 'ref' to branch
                        "branch" |
                        "ref" => reference = GitReference::Branch(v.clone()),

                        "rev" => reference = GitReference::Rev(v.clone()),
                        "tag" => reference = GitReference::Tag(v.clone()),
                        _ => {}
                    }
                }
                url.query = None;
                let precise = mem::replace(&mut url.fragment, None);
                SourceId::for_git(&url, reference)
                         .with_precise(precise)
            },
            "registry" => {
                let url = url.to_url().unwrap();
                SourceId::new(Kind::Registry, url)
                         .with_precise(Some("locked".to_string()))
            }
            "path" => {
                let url = url.to_url().unwrap();
                SourceId::new(Kind::Path, url)
            }
            _ => panic!("Unsupported serialized SourceId")
        }
    }

    pub fn to_url(&self) -> String {
        match *self.inner {
            SourceIdInner { kind: Kind::Path, ref url, .. } => {
                format!("path+{}", url)
            }
            SourceIdInner {
                kind: Kind::Git(ref reference), ref url, ref precise, ..
            } => {
                let ref_str = url_ref(reference);

                let precise_str = if precise.is_some() {
                    format!("#{}", precise.as_ref().unwrap())
                } else {
                    "".to_string()
                };

                format!("git+{}{}{}", url, ref_str, precise_str)
            },
            SourceIdInner { kind: Kind::Registry, ref url, .. } => {
                format!("registry+{}", url)
            }
        }
    }

    // Pass absolute path
    pub fn for_path(path: &Path) -> CargoResult<SourceId> {
        let url = try!(path.to_url().map_err(human));
        Ok(SourceId::new(Kind::Path, url))
    }

    pub fn for_git(url: &Url, reference: GitReference) -> SourceId {
        SourceId::new(Kind::Git(reference), url.clone())
    }

    pub fn for_registry(url: &Url) -> SourceId {
        SourceId::new(Kind::Registry, url.clone())
    }

    /// Returns the `SourceId` corresponding to the main repository.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a `.cargo/config`.
    pub fn for_central(config: &Config) -> CargoResult<SourceId> {
        Ok(SourceId::for_registry(&try!(RegistrySource::url(config))))
    }

    pub fn url(&self) -> &Url { &self.inner.url }
    pub fn is_path(&self) -> bool { self.inner.kind == Kind::Path }
    pub fn is_registry(&self) -> bool { self.inner.kind == Kind::Registry }

    pub fn is_git(&self) -> bool {
        match self.inner.kind {
            Kind::Git(_) => true,
            _ => false
        }
    }

    /// Creates an implementation of `Source` corresponding to this ID.
    pub fn load<'a>(&self, config: &'a Config) -> Box<Source+'a> {
        trace!("loading SourceId; {}", self);
        match self.inner.kind {
            Kind::Git(..) => Box::new(GitSource::new(self, config)),
            Kind::Path => {
                let path = match self.inner.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => panic!("path sources cannot be remote"),
                };
                Box::new(PathSource::new(&path, self, config))
            }
            Kind::Registry => Box::new(RegistrySource::new(self, config)),
        }
    }

    pub fn precise(&self) -> Option<&str> {
        self.inner.precise.as_ref().map(|s| &s[..])
    }

    pub fn git_reference(&self) -> Option<&GitReference> {
        match self.inner.kind {
            Kind::Git(ref s) => Some(s),
            _ => None,
        }
    }

    pub fn with_precise(&self, v: Option<String>) -> SourceId {
        SourceId {
            inner: Arc::new(SourceIdInner {
                precise: v,
                .. (*self.inner).clone()
            }),
        }
    }

    pub fn is_default_registry(&self) -> bool {
        match self.inner.kind {
            Kind::Registry => {}
            _ => return false,
        }
        self.inner.url.to_string() == RegistrySource::default_url()
    }
}

impl PartialEq for SourceId {
    fn eq(&self, other: &SourceId) -> bool {
        (*self.inner).eq(&*other.inner)
    }
}

impl PartialOrd for SourceId {
    fn partial_cmp(&self, other: &SourceId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceId {
    fn cmp(&self, other: &SourceId) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl Encodable for SourceId {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        if self.is_path() {
            s.emit_option_none()
        } else {
           self.to_url().encode(s)
        }
    }
}

impl Decodable for SourceId {
    fn decode<D: Decoder>(d: &mut D) -> Result<SourceId, D::Error> {
        let string: String = Decodable::decode(d).ok().expect("Invalid encoded SourceId");
        Ok(SourceId::from_url(string))
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self.inner {
            SourceIdInner { kind: Kind::Path, ref url, .. } => {
                fmt::Display::fmt(url, f)
            }
            SourceIdInner { kind: Kind::Git(ref reference), ref url,
                            ref precise, .. } => {
                try!(write!(f, "{}{}", url, url_ref(reference)));

                if let Some(ref s) = *precise {
                    let len = cmp::min(s.len(), 8);
                    try!(write!(f, "#{}", &s[..len]));
                }
                Ok(())
            }
            SourceIdInner { kind: Kind::Registry, ref url, .. } => {
                write!(f, "registry {}", url)
            }
        }
    }
}

// This custom implementation handles situations such as when two git sources
// point at *almost* the same URL, but not quite, even when they actually point
// to the same repository.
impl PartialEq for SourceIdInner {
    fn eq(&self, other: &SourceIdInner) -> bool {
        if self.kind != other.kind { return false }
        if self.url == other.url { return true }

        match (&self.kind, &other.kind) {
            (&Kind::Git(ref ref1), &Kind::Git(ref ref2)) => {
                ref1 == ref2 && self.canonical_url == other.canonical_url
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
            (&Kind::Git(ref ref1), &Kind::Git(ref ref2)) => {
                (ref1, &self.canonical_url).cmp(&(ref2, &other.canonical_url))
            }
            _ => self.kind.cmp(&other.kind)
        }
    }
}

impl hash::Hash for SourceId {
    fn hash<S: hash::Hasher>(&self, into: &mut S) {
        self.inner.kind.hash(into);
        match *self.inner {
            SourceIdInner { kind: Kind::Git(..), ref canonical_url, .. } => {
                canonical_url.hash(into)
            }
            _ => self.inner.url.hash(into),
        }
    }
}

fn url_ref(r: &GitReference) -> String {
    match r.to_ref_string() {
        None => "".to_string(),
        Some(s) => format!("?{}", s),
    }
}

impl GitReference {
    pub fn to_ref_string(&self) -> Option<String> {
        match *self {
            GitReference::Branch(ref s) => {
                if *s == "master" {
                    None
                } else {
                    Some(format!("branch={}", s))
                }
            }
            GitReference::Tag(ref s) => Some(format!("tag={}", s)),
            GitReference::Rev(ref s) => Some(format!("rev={}", s)),
        }
    }
}

pub struct SourceMap<'src> {
    map: HashMap<SourceId, Box<Source+'src>>
}

pub type Sources<'a, 'src> = Values<'a, SourceId, Box<Source+'src>>;

pub struct SourcesMut<'a, 'src: 'a> {
    inner: IterMut<'a, SourceId, Box<Source + 'src>>,
}

impl<'src> SourceMap<'src> {
    pub fn new() -> SourceMap<'src> {
        SourceMap {
            map: HashMap::new()
        }
    }

    pub fn contains(&self, id: &SourceId) -> bool {
        self.map.contains_key(id)
    }

    pub fn get(&self, id: &SourceId) -> Option<&(Source+'src)> {
        let source = self.map.get(id);

        source.map(|s| {
            let s: &(Source+'src) = &**s;
            s
        })
    }

    pub fn get_mut(&mut self, id: &SourceId) -> Option<&mut (Source+'src)> {
        self.map.get_mut(id).map(|s| {
            let s: &mut (Source+'src) = &mut **s;
            s
        })
    }

    pub fn get_by_package_id(&self, pkg_id: &PackageId) -> Option<&(Source+'src)> {
        self.get(pkg_id.source_id())
    }

    pub fn insert(&mut self, id: &SourceId, source: Box<Source+'src>) {
        self.map.insert(id.clone(), source);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn sources<'a>(&'a self) -> Sources<'a, 'src> {
        self.map.values()
    }

    pub fn sources_mut<'a>(&'a mut self) -> SourcesMut<'a, 'src> {
        SourcesMut { inner: self.map.iter_mut() }
    }
}

impl<'a, 'src> Iterator for SourcesMut<'a, 'src> {
    type Item = (&'a SourceId, &'a mut (Source + 'src));
    fn next(&mut self) -> Option<(&'a SourceId, &'a mut (Source + 'src))> {
        self.inner.next().map(|(a, b)| (a, &mut **b))
    }
}

/// List of `Source` implementors. `SourceSet` itself implements `Source`.
pub struct SourceSet<'src> {
    sources: Vec<Box<Source+'src>>
}

impl<'src> SourceSet<'src> {
    pub fn new(sources: Vec<Box<Source+'src>>) -> SourceSet<'src> {
        SourceSet { sources: sources }
    }
}

impl<'src> Registry for SourceSet<'src> {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();

        for source in self.sources.iter_mut() {
            ret.extend(try!(source.query(name)).into_iter());
        }

        Ok(ret)
    }
}

impl<'src> Source for SourceSet<'src> {
    fn update(&mut self) -> CargoResult<()> {
        for source in self.sources.iter_mut() {
            try!(source.update());
        }

        Ok(())
    }

    fn download(&mut self, packages: &[PackageId]) -> CargoResult<()> {
        for source in self.sources.iter_mut() {
            try!(source.download(packages));
        }

        Ok(())
    }

    fn get(&self, packages: &[PackageId]) -> CargoResult<Vec<Package>> {
        let mut ret = Vec::new();

        for source in self.sources.iter() {
            ret.extend(try!(source.get(packages)).into_iter());
        }

        Ok(ret)
    }

    fn fingerprint(&self, id: &Package) -> CargoResult<String> {
        let mut ret = String::new();
        for source in self.sources.iter() {
            ret.push_str(&try!(source.fingerprint(id))[..]);
        }
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceId, Kind, GitReference};
    use util::ToUrl;

    #[test]
    fn github_sources_equal() {
        let loc = "https://github.com/foo/bar".to_url().unwrap();
        let master = Kind::Git(GitReference::Branch("master".to_string()));
        let s1 = SourceId::new(master.clone(), loc);

        let loc = "git://github.com/foo/bar".to_url().unwrap();
        let s2 = SourceId::new(master, loc.clone());

        assert_eq!(s1, s2);

        let foo = Kind::Git(GitReference::Branch("foo".to_string()));
        let s3 = SourceId::new(foo, loc);
        assert!(s1 != s3);
    }
}
