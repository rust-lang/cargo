use std::fmt;
use std::fmt::{Show, Formatter};
use serialize::{Decodable, Decoder, Encodable, Encoder};

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

#[deriving(Encodable, Decodable, Clone, Eq, Hash)]
pub struct SourceId {
    pub kind: SourceKind,
    pub location: Location,
}

impl Show for Location {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Local(ref p) => write!(f, "file:{}", p.display()),
            Remote(ref u) => write!(f, "{}", u),
        }
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

impl Show for SourceId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            SourceId { kind: PathKind, ref location } => {
                try!(write!(f, "{}", location))
            },
            SourceId { kind: GitKind(ref reference), ref location } => {
                try!(write!(f, "{}", location));
                if reference.as_slice() != "master" {
                    try!(write!(f, "#ref={}", reference));
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

impl SourceId {
    pub fn new(kind: SourceKind, location: Location) -> SourceId {
        SourceId { kind: kind, location: location }
    }

    // Pass absolute path
    pub fn for_path(path: &Path) -> SourceId {
        SourceId::new(PathKind, Local(path.clone()))
    }

    pub fn for_git(url: &Url, reference: &str) -> SourceId {
        SourceId::new(GitKind(reference.to_string()), Remote(url.clone()))
    }

    pub fn for_central() -> SourceId {
        SourceId::new(RegistryKind,
                      Remote(Url::parse("https://example.com").unwrap()))
    }

    pub fn get_location<'a>(&'a self) -> &'a Location {
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

    #[test]
    fn github_sources_equal() {
        let loc = Remote(from_str("https://github.com/foo/bar").unwrap());
        let s1 = SourceId::new(GitKind("master".to_string()), loc);

        let loc = Remote(from_str("git://github.com/foo/bar").unwrap());
        let mut s2 = SourceId::new(GitKind("master".to_string()), loc);

        assert_eq!(s1, s2);

        s2.kind = GitKind("foo".to_string());
        assert!(s1 != s2);
    }
}
