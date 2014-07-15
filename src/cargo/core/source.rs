use std::collections::HashMap;
use std::collections::hashmap::Values;
use std::fmt;
use std::fmt::{Show, Formatter};
use std::hash;
use std::c_str::CString;
use std::cmp::Ordering;
use serialize::{Decodable, Decoder, Encodable, Encoder};

use url;
use url::Url;

use core::{Summary, Package, PackageId};
use sources::{PathSource, GitSource};
use sources::git;
use util::{Config, CargoResult, CargoError};
use util::errors::human;

/// A Source finds and downloads remote packages based on names and
/// versions.
pub trait Source {
    /// The update method performs any network operations required to
    /// get the entire list of all names, versions and dependencies of
    /// packages managed by the Source.
    fn update(&mut self) -> CargoResult<()>;

    /// The list method lists all names, versions and dependencies of
    /// packages managed by the source. It assumes that `update` has
    /// already been called and no additional network operations are
    /// required.
    fn list(&self) -> CargoResult<Vec<Summary>>;

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

#[deriving(Clone, PartialEq, Eq, Hash)]
pub enum Location {
    Local(Path),
    Remote(Url),
}

type Error = Box<CargoError + Send>;

impl<E, D: Decoder<E>> Decodable<D, E> for Location {
    fn decode(d: &mut D) -> Result<Location, E> {
        let url: String  = raw_try!(Decodable::decode(d));
        Ok(Location::parse(url.as_slice()).unwrap())
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for Location {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        self.to_string().encode(e)
    }
}

#[deriving(Clone, Eq)]
pub struct SourceId {
    pub location: Location,
    pub kind: SourceKind,
    // e.g. the exact git revision of the specified branch for a Git Source
    pub precise: Option<String>
}

impl Show for Location {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Local(ref p) => write!(f, "file:{}", p.display()),
            Remote(ref u) => write!(f, "{}", u),
        }
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

impl Location {
    pub fn parse(s: &str) -> CargoResult<Location> {
        if s.starts_with("file:") {
            Ok(Local(Path::new(s.slice_from(5))))
        } else {
            Url::parse(s).map(Remote).map_err(|e| {
                human(format!("invalid url `{}`: `{}", s, e))
            })
        }
    }
}

impl<'a> ToCStr for &'a Location {
    fn to_c_str(&self) -> CString {
        match **self {
            Local(ref p) => p.to_c_str(),
            Remote(ref u) => u.to_string().to_c_str(),
        }
    }

    unsafe fn to_c_str_unchecked(&self) -> CString { self.to_c_str() }
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
            SourceId { kind: PathKind, ref location, .. } => {
                try!(write!(f, "{}", location))
            },
            SourceId { kind: GitKind(ref reference), ref location, ref precise, .. } => {
                try!(write!(f, "{}", location));
                if reference.as_slice() != "master" {
                    try!(write!(f, "?ref={}", reference));
                }

                if precise.is_some() {
                    try!(write!(f, "#{}", precise.get_ref()));
                }
            },
            SourceId { kind: RegistryKind, .. } => {
                // TODO: Central registry vs. alternates
                try!(write!(f, "the package registry"));
            }
        }

        Ok(())
    }
}

// This custom implementation handles situations such as when two git sources
// point at *almost* the same URL, but not quite, even when they actually point
// to the same repository.
impl PartialEq for SourceId {
    fn eq(&self, other: &SourceId) -> bool {
        if self.kind != other.kind { return false }
        if self.location == other.location { return true }

        match (&self.kind, &other.kind, &self.location, &other.location) {
            (&GitKind(..), &GitKind(..),
             &Remote(ref u1), &Remote(ref u2)) => {
                git::canonicalize_url(u1.to_string().as_slice()) ==
                    git::canonicalize_url(u2.to_string().as_slice())
            }
            _ => false,
        }
    }
}

impl<S: hash::Writer> hash::Hash<S> for SourceId {
    fn hash(&self, into: &mut S) {
        match *self {
            SourceId {
                kind: ref kind @ GitKind(..),
                location: Remote(ref url),
                precise: None
            } => {
                kind.hash(into);
                git::canonicalize_url(url.to_string().as_slice()).hash(into);
            }
            _ => {
                self.kind.hash(into);
                self.location.hash(into);
            }
        }
    }
}

impl SourceId {
    pub fn new(kind: SourceKind, location: Location) -> SourceId {
        SourceId { kind: kind, location: location, precise: None }
    }

    pub fn from_url(string: String) -> SourceId {
        let mut parts = string.as_slice().splitn('+', 1);
        let kind = parts.nth(0).unwrap();
        let mut url = Url::parse(parts.nth(0).unwrap()).ok().expect("Invalid URL");

        match kind {
            "git" => {
                let reference = {
                    url.path.query.iter()
                        .find(|&&(ref k, ref v)| k.as_slice() == "ref")
                        .map(|&(ref k, ref v)| v.to_string())
                        .unwrap_or("master".to_string())
                        .to_string()
                };

                url.path.query = url.path.query.iter()
                    .filter(|&&(ref k,_)| k.as_slice() != "ref")
                    .map(|q| q.clone())
                    .collect();

                let precise = url.path.fragment.clone();
                url.path.fragment = None;

                SourceId::for_git(&url, reference.as_slice(), precise)
            },
            _ => fail!("Unsupported serialized SourceId")
        }
    }

    pub fn to_url(&self) -> String {
        match *self {
            SourceId { kind: PathKind, ref location, .. } => {
                fail!("Path sources are not included in the lockfile, so this is unimplemented");
            },
            SourceId { kind: GitKind(ref reference), ref location, ref precise, .. } => {
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

                format!("git+{}{}{}", location, ref_str, precise_str)
            },
            SourceId { kind: RegistryKind, .. } => {
                // TODO: Central registry vs. alternates
                "registry+https://crates.io/".to_string()
            }
        }
    }

    // Pass absolute path
    pub fn for_path(path: &Path) -> SourceId {
        SourceId::new(PathKind, Local(path.clone()))
    }

    pub fn for_git(url: &Url, reference: &str, precise: Option<String>) -> SourceId {
        let mut id = SourceId::new(GitKind(reference.to_string()), Remote(url.clone()));
        if precise.is_some() {
            id = id.with_precise(precise.unwrap());
        }

        id
    }

    pub fn for_central() -> SourceId {
        SourceId::new(RegistryKind,
                      Remote(Url::parse("https://example.com").unwrap()))
    }

    pub fn get_location(&self) -> &Location {
        &self.location
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
                let path = match self.location {
                    Local(ref p) => p,
                    Remote(..) => fail!("path sources cannot be remote"),
                };
                box PathSource::new(path, self) as Box<Source>
            },
            RegistryKind => unimplemented!()
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
}

pub struct SourceSet {
    sources: Vec<Box<Source>>
}

impl SourceSet {
    pub fn new(sources: Vec<Box<Source>>) -> SourceSet {
        SourceSet { sources: sources }
    }
}

impl Source for SourceSet {
    fn update(&mut self) -> CargoResult<()> {
        for source in self.sources.mut_iter() {
            try!(source.update());
        }

        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        let mut ret = Vec::new();

        for source in self.sources.iter() {
            ret.push_all(try!(source.list()).as_slice());
        }

        Ok(ret)
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
    use super::{SourceId, Remote, GitKind};
    use util::ToUrl;

    #[test]
    fn github_sources_equal() {
        let loc = Remote("https://github.com/foo/bar".to_url().unwrap());
        let s1 = SourceId::new(GitKind("master".to_string()), loc);

        let loc = Remote("git://github.com/foo/bar".to_url().unwrap());
        let mut s2 = SourceId::new(GitKind("master".to_string()), loc);

        assert_eq!(s1, s2);

        s2.kind = GitKind("foo".to_string());
        assert!(s1 != s2);
    }
}
