use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hashmap::{Values, MutEntries};
use std::fmt::{mod, Show, Formatter};
use std::hash;
use std::iter;
use std::mem;
use serialize::{Decodable, Decoder, Encodable, Encoder};

use url::Url;

use core::{Summary, Package, PackageId, Registry, Dependency};
use sources::{PathSource, GitSource, DummyRegistrySource};
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
    fn download(&self, packages: &[PackageId]) -> CargoResult<()>;

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
pub enum SourceKind {
    /// GitKind(<git reference>) represents a git repository
    GitKind(String),
    /// represents a local path
    PathKind,
    /// represents the central registry
    RegistryKind
}

type Error = Box<CargoError + Send>;

#[deriving(Clone, Eq)]
pub struct SourceId {
    pub url: Url,
    pub kind: SourceKind,
    // e.g. the exact git revision of the specified branch for a Git Source
    pub precise: Option<String>
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
        match *self {
            SourceId { kind: PathKind, ref url, .. } => url.fmt(f),
            SourceId { kind: GitKind(ref reference), ref url, ref precise, .. } => {
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
            SourceId { kind: RegistryKind, .. } => {
                // TODO: Central registry vs. alternates
                write!(f, "the package registry")
            }
        }
    }
}

// This custom implementation handles situations such as when two git sources
// point at *almost* the same URL, but not quite, even when they actually point
// to the same repository.
impl PartialEq for SourceId {
    fn eq(&self, other: &SourceId) -> bool {
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
        self.kind.hash(into);
        match *self {
            SourceId { kind: GitKind(..), ref url, .. } => {
                git::canonicalize_url(url).hash(into)
            }
            _ => self.url.hash(into),
        }
    }
}

impl SourceId {
    pub fn new(kind: SourceKind, url: Url) -> SourceId {
        SourceId { kind: kind, url: url, precise: None }
    }

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
                SourceId::for_git(&url, reference.as_slice(), precise)
            },
            "registry" => SourceId::for_central(),
            "path" => SourceId::for_path(&Path::new(url.slice_from(5))).unwrap(),
            _ => fail!("Unsupported serialized SourceId")
        }
    }

    pub fn to_url(&self) -> String {
        match *self {
            SourceId { kind: PathKind, .. } => {
                fail!("Path sources are not included in the lockfile, \
                       so this is unimplemented")
            },
            SourceId {
                kind: GitKind(ref reference), ref url, ref precise, ..
            } => {
                let ref_str = if reference.as_slice() != "master" {
                    format!("?ref={}", reference)
                } else {
                    "".to_string()
                };

                let precise_str = if precise.is_some() {
                    format!("#{}", precise.get_ref())
                } else {
                    "".to_string()
                };

                format!("git+{}{}{}", url, ref_str, precise_str)
            },
            SourceId { kind: RegistryKind, .. } => {
                // TODO: Central registry vs. alternates
                "registry+https://crates.io/".to_string()
            }
        }
    }

    // Pass absolute path
    pub fn for_path(path: &Path) -> CargoResult<SourceId> {
        let url = try!(Url::from_file_path(path).map_err(|()| {
            human(format!("not a valid path for a URL: {}", path.display()))
        }));
        Ok(SourceId::new(PathKind, url))
    }

    pub fn for_git(url: &Url, reference: &str, precise: Option<String>) -> SourceId {
        let mut id = SourceId::new(GitKind(reference.to_string()), url.clone());
        if precise.is_some() {
            id = id.with_precise(precise.unwrap());
        }

        id
    }

    pub fn for_central() -> SourceId {
        SourceId::new(RegistryKind,
                      "https://example.com".to_url().unwrap())
    }

    pub fn get_url(&self) -> &Url {
        &self.url
    }

    pub fn is_path(&self) -> bool {
        self.kind == PathKind
    }

    pub fn is_git(&self) -> bool {
        match self.kind {
            GitKind(_) => true,
            _ => false
        }
    }

    pub fn load(&self, config: &mut Config) -> Box<Source> {
        log!(5, "loading SourceId; {}", self);
        match self.kind {
            GitKind(..) => box GitSource::new(self, config) as Box<Source>,
            PathKind => {
                let path = match self.url.to_file_path() {
                    Ok(p) => p,
                    Err(()) => fail!("path sources cannot be remote"),
                };
                box PathSource::new(&path, self) as Box<Source>
            },
            RegistryKind => box DummyRegistrySource::new(self) as Box<Source>,
        }
    }

    pub fn with_precise(&self, v: String) -> SourceId {
        SourceId {
            precise: Some(v),
            .. self.clone()
        }
    }
}

pub struct SourceMap {
    map: HashMap<SourceId, Box<Source>>
}

pub type Sources<'a> = Values<'a, SourceId, Box<Source>>;
pub type SourcesMut<'a> = iter::Map<'static, (&'a SourceId, &'a mut Box<Source>),
                                    &'a mut Source,
                                    MutEntries<'a, SourceId, Box<Source>>>;

impl SourceMap {
    pub fn new() -> SourceMap {
        SourceMap {
            map: HashMap::new()
        }
    }

    pub fn contains(&self, id: &SourceId) -> bool {
        self.map.contains_key(id)
    }

    pub fn get(&self, id: &SourceId) -> Option<&Source> {
        let source = self.map.find(id);

        source.map(|s| {
            let s: &Source = *s;
            s
        })
    }

    pub fn get_mut(&mut self, id: &SourceId) -> Option<&mut Source> {
        self.map.find_mut(id).map(|s| {
            let s: &mut Source = *s;
            s
        })
    }

    pub fn get_by_package_id(&self, pkg_id: &PackageId) -> Option<&Source> {
        self.get(pkg_id.get_source_id())
    }

    pub fn insert(&mut self, id: &SourceId, source: Box<Source>) {
        self.map.insert(id.clone(), source);
    }

    pub fn len(&self) -> uint {
        self.map.len()
    }

    pub fn sources(&self) -> Sources {
        self.map.values()
    }

    pub fn sources_mut(&mut self) -> SourcesMut {
        self.map.mut_iter().map(|(_, v)| { let s: &mut Source = *v; s })
    }
}

pub struct SourceSet {
    sources: Vec<Box<Source>>
}

impl SourceSet {
    pub fn new(sources: Vec<Box<Source>>) -> SourceSet {
        SourceSet { sources: sources }
    }
}

impl Registry for SourceSet {
    fn query(&mut self, name: &Dependency) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();

        for source in self.sources.mut_iter() {
            ret.push_all_move(try!(source.query(name)));
        }

        Ok(ret)
    }
}

impl Source for SourceSet {
    fn update(&mut self) -> CargoResult<()> {
        for source in self.sources.mut_iter() {
            try!(source.update());
        }

        Ok(())
    }

    fn download(&self, packages: &[PackageId]) -> CargoResult<()> {
        for source in self.sources.iter() {
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
        let mut s2 = SourceId::new(GitKind("master".to_string()), loc);

        assert_eq!(s1, s2);

        s2.kind = GitKind("foo".to_string());
        assert!(s1 != s2);
    }
}
