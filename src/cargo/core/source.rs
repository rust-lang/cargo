use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hashmap::{Values, MutEntries};
use std::fmt::{mod, Show, Formatter};
use std::hash;
use std::iter;
use std::mem;
use std::sync::Arc;
use serialize::{Decodable, Decoder, Encodable, Encoder};

use url::Url;

use core::{Summary, Package, PackageId, Registry, Dependency};
use sources::{PathSource, GitSource, RegistrySource};
use sources::git;
use util::{human, Config, CargoResult, CargoError, ToUrl};

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

#[deriving(Encodable, Decodable, Show, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum SourceKind {
    /// GitKind(<git reference>) represents a git repository
    GitKind(String),
    /// represents a local path
    PathKind,
    /// represents the central registry
    RegistryKind,
}

type Error = Box<CargoError + Send>;

/// Unique identifier for a source of packages.
#[deriving(Clone, Eq)]
pub struct SourceId {
    inner: Arc<SourceIdInner>,
}

#[deriving(Eq, Clone)]
struct SourceIdInner {
    url: Url,
    kind: SourceKind,
    // e.g. the exact git revision of the specified branch for a Git Source
    precise: Option<String>
}

impl SourceId {
    fn new(kind: SourceKind, url: Url) -> SourceId {
        SourceId {
            inner: Arc::new(SourceIdInner {
                kind: kind,
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
        let mut parts = string.as_slice().splitn(1, '+');
        let kind = parts.next().unwrap();
        let url = parts.next().unwrap();

        match kind {
            "git" => {
                let mut url = url.to_url().unwrap();
                let mut reference = "master".to_string();
                let pairs = url.query_pairs().unwrap_or(Vec::new());
                for &(ref k, ref v) in pairs.iter() {
                    if k.as_slice() == "ref" {
                        reference = v.clone();
                    }
                }
                url.query = None;
                let precise = mem::replace(&mut url.fragment, None);
                SourceId::for_git(&url, reference.as_slice())
                         .with_precise(precise)
            },
            "registry" => {
                let url = url.to_url().unwrap();
                SourceId::new(RegistryKind, url)
                         .with_precise(Some("locked".to_string()))
            }
            "path" => SourceId::for_path(&Path::new(url.slice_from(5))).unwrap(),
            _ => panic!("Unsupported serialized SourceId")
        }
    }

    pub fn to_url(&self) -> String {
        match *self.inner {
            SourceIdInner { kind: PathKind, .. } => {
                panic!("Path sources are not included in the lockfile, \
                       so this is unimplemented")
            },
            SourceIdInner {
                kind: GitKind(ref reference), ref url, ref precise, ..
            } => {
                let ref_str = if reference.as_slice() != "master" {
                    format!("?ref={}", reference)
                } else {
                    "".to_string()
                };

                let precise_str = if precise.is_some() {
                    format!("#{}", precise.as_ref().unwrap())
                } else {
                    "".to_string()
                };

                format!("git+{}{}{}", url, ref_str, precise_str)
            },
            SourceIdInner { kind: RegistryKind, ref url, .. } => {
                format!("registry+{}", url)
            }
        }
    }

    // Pass absolute path
    pub fn for_path(path: &Path) -> CargoResult<SourceId> {
        let url = try!(path.to_url().map_err(human));
        Ok(SourceId::new(PathKind, url))
    }

    pub fn for_git(url: &Url, reference: &str) -> SourceId {
        SourceId::new(GitKind(reference.to_string()), url.clone())
    }

    pub fn for_registry(url: &Url) -> SourceId {
        SourceId::new(RegistryKind, url.clone())
    }

    /// Returns the `SourceId` corresponding to the main repository.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a `.cargo/config`.
    pub fn for_central() -> CargoResult<SourceId> {
        Ok(SourceId::for_registry(&try!(RegistrySource::url())))
    }

    pub fn get_url(&self) -> &Url { &self.inner.url }
    pub fn is_path(&self) -> bool { self.inner.kind == PathKind }
    pub fn is_registry(&self) -> bool { self.inner.kind == RegistryKind }

    pub fn is_git(&self) -> bool {
        match self.inner.kind {
            GitKind(_) => true,
            _ => false
        }
    }

    /// Creates an implementation of `Source` corresponding to this ID.
    pub fn load<'a>(&self, config: &'a mut Config) -> Box<Source+'a> {
        log!(5, "loading SourceId; {}", self);
        match self.inner.kind {
            GitKind(..) => box GitSource::new(self, config) as Box<Source+'a>,
            PathKind => {
                let path = match self.inner.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => panic!("path sources cannot be remote"),
                };
                box PathSource::new(&path, self) as Box<Source>
            },
            RegistryKind => {
                box RegistrySource::new(self, config) as Box<Source+'a>
            }
        }
    }

    pub fn get_precise(&self) -> Option<&str> {
        self.inner.precise.as_ref().map(|s| s.as_slice())
    }

    pub fn git_reference(&self) -> Option<&str> {
        match self.inner.kind {
            GitKind(ref s) => Some(s.as_slice()),
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
}

impl PartialEq for SourceId {
    fn eq(&self, other: &SourceId) -> bool {
        (*self.inner).eq(&*other.inner)
    }
}

impl PartialOrd for SourceId {
    fn partial_cmp(&self, other: &SourceId) -> Option<Ordering> {
        self.to_string().partial_cmp(&other.to_string())
    }
}

impl Ord for SourceId {
    fn cmp(&self, other: &SourceId) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for SourceId {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        if self.is_path() {
            s.emit_option_none()
        } else {
           self.to_url().encode(s)
        }
    }
}

impl<E, D: Decoder<E>> Decodable<D, E> for SourceId {
    fn decode(d: &mut D) -> Result<SourceId, E> {
        let string: String = Decodable::decode(d).ok().expect("Invalid encoded SourceId");
        Ok(SourceId::from_url(string))
    }
}

impl Show for SourceId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self.inner {
            SourceIdInner { kind: PathKind, ref url, .. } => url.fmt(f),
            SourceIdInner { kind: GitKind(ref reference), ref url,
                            ref precise, .. } => {
                try!(write!(f, "{}", url));
                if reference.as_slice() != "master" {
                    try!(write!(f, "?ref={}", reference));
                }

                match *precise {
                    Some(ref s) => {
                        try!(write!(f, "#{}", s.as_slice().slice_to(8)));
                    }
                    None => {}
                }
                Ok(())
            },
            SourceIdInner { kind: RegistryKind, ref url, .. } => {
                let default = RegistrySource::url().ok();
                if default.as_ref() == Some(url) {
                    write!(f, "the package registry")
                } else {
                    write!(f, "registry {}", url)
                }
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

        match (&self.kind, &other.kind, &self.url, &other.url) {
            (&GitKind(ref ref1), &GitKind(ref ref2), u1, u2) => {
                ref1 == ref2 &&
                git::canonicalize_url(u1) == git::canonicalize_url(u2)
            }
            _ => false,
        }
    }
}

impl<S: hash::Writer> hash::Hash<S> for SourceId {
    fn hash(&self, into: &mut S) {
        self.inner.kind.hash(into);
        match *self.inner {
            SourceIdInner { kind: GitKind(..), ref url, .. } => {
                git::canonicalize_url(url).hash(into)
            }
            _ => self.inner.url.hash(into),
        }
    }
}

pub struct SourceMap<'a> {
    map: HashMap<SourceId, Box<Source+'a>>
}

pub type Sources<'a> = Values<'a, SourceId, Box<Source+'a>>;
pub type SourcesMut<'a> = iter::Map<'static, (&'a SourceId, &'a mut Box<Source+'a>),
                                    &'a mut Source+'a,
                                    MutEntries<'a, SourceId, Box<Source+'a>>>;

impl<'a> SourceMap<'a> {
    pub fn new() -> SourceMap<'a> {
        SourceMap {
            map: HashMap::new()
        }
    }

    pub fn contains(&self, id: &SourceId) -> bool {
        self.map.contains_key(id)
    }

    pub fn get(&self, id: &SourceId) -> Option<&Source+'a> {
        let source = self.map.find(id);

        source.map(|s| {
            let s: &Source+'a = &**s;
            s
        })
    }

    pub fn get_mut(&mut self, id: &SourceId) -> Option<&mut Source+'a> {
        self.map.find_mut(id).map(|s| {
            let s: &mut Source+'a = &mut **s;
            s
        })
    }

    pub fn get_by_package_id(&self, pkg_id: &PackageId) -> Option<&Source+'a> {
        self.get(pkg_id.get_source_id())
    }

    pub fn insert(&mut self, id: &SourceId, source: Box<Source+'a>) {
        self.map.insert(id.clone(), source);
    }

    pub fn len(&self) -> uint {
        self.map.len()
    }

    pub fn sources(&'a self) -> Sources<'a> {
        self.map.values()
    }

    pub fn sources_mut(&'a mut self) -> SourcesMut<'a> {
        self.map.iter_mut().map(|(_, v)| { let s: &mut Source+'a = &mut **v; s })
    }
}

/// List of `Source` implementors. `SourceSet` itself implements `Source`.
pub struct SourceSet<'a> {
    sources: Vec<Box<Source+'a>>
}

impl<'a> SourceSet<'a> {
    pub fn new(sources: Vec<Box<Source+'a>>) -> SourceSet<'a> {
        SourceSet { sources: sources }
    }
}

impl<'a> Registry for SourceSet<'a> {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();

        for source in self.sources.iter_mut() {
            ret.extend(try!(source.query(name)).into_iter());
        }

        Ok(ret)
    }
}

impl<'a> Source for SourceSet<'a> {
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
            ret.push_all(try!(source.get(packages)).as_slice());
        }

        Ok(ret)
    }

    fn fingerprint(&self, id: &Package) -> CargoResult<String> {
        let mut ret = String::new();
        for source in self.sources.iter() {
            ret.push_str(try!(source.fingerprint(id)).as_slice());
        }
        return Ok(ret);
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceId, GitKind};
    use util::ToUrl;

    #[test]
    fn github_sources_equal() {
        let loc = "https://github.com/foo/bar".to_url().unwrap();
        let s1 = SourceId::new(GitKind("master".to_string()), loc);

        let loc = "git://github.com/foo/bar".to_url().unwrap();
        let s2 = SourceId::new(GitKind("master".to_string()), loc.clone());

        assert_eq!(s1, s2);

        let s3 = SourceId::new(GitKind("foo".to_string()), loc);
        assert!(s1 != s3);
    }
}
