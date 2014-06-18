use url;
use url::Url;
use core::{Summary,Package,PackageId};
use sources::{PathSource,GitSource};
use util::{Config,CargoResult};

/**
 * A Source finds and downloads remote packages based on names and
 * versions.
 */
pub trait Source {
    /**
     * The update method performs any network operations required to
     * get the entire list of all names, versions and dependencies of
     * packages managed by the Source.
     */
    fn update(&self) -> CargoResult<()>;

    /**
     * The list method lists all names, versions and dependencies of
     * packages managed by the source. It assumes that `update` has
     * already been called and no additional network operations are
     * required.
     */
    fn list(&self) -> CargoResult<Vec<Summary>>;

    /**
     * The download method fetches the full package for each name and
     * version specified.
     */
    fn download(&self, packages: &[PackageId]) -> CargoResult<()>;

    /**
     * The get method returns the Path of each specified package on the
     * local file system. It assumes that `download` was already called,
     * and that the packages are already locally available on the file
     * system.
     */
    fn get(&self, packages: &[PackageId]) -> CargoResult<Vec<Package>>;
}

#[deriving(Show,Clone,PartialEq)]
pub enum SourceKind {
    /// GitKind(<git reference>) represents a git repository
    GitKind(String),
    /// represents a local path
    PathKind,
    /// represents the central registry
    RegistryKind
}

#[deriving(Show,Clone,PartialEq)]
pub struct SourceId {
    pub kind: SourceKind,
    pub url: Url
}

impl SourceId {
    pub fn new(kind: SourceKind, url: Url) -> SourceId {
        SourceId { kind: kind, url: url }
    }

    // Pass absolute path
    pub fn for_path(path: &Path) -> SourceId {
        // TODO: use proper path -> URL
        SourceId::new(PathKind, url::from_str(format!("file://{}", path.display()).as_slice()).unwrap())
    }

    pub fn for_git(url: &Url, reference: &str) -> SourceId {
        SourceId::new(GitKind(reference.to_str()), url.clone())
    }

    pub fn for_central() -> SourceId {
        SourceId::new(RegistryKind, url::from_str(format!("https://example.com").as_slice()).unwrap())
    }

    pub fn get_url<'a>(&'a self) -> &'a Url {
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

    /*
    let git_sources: Vec<Box<Source>> = try!(result::collect(package.get_sources().iter().map(|source_id: &SourceId| {
        match source_id.kind {
            GitKind(ref reference) => {
                let remote = GitRemote::new(source_id.url.clone(), false);
                let home = try!(os::homedir().require(simple_human("Cargo couldn't find a home directory")));
                let git = home.join(".cargo").join("git");
                let ident = url_to_path_ident(&source_id.url);

                // .cargo/git/db
                // .cargo/git/checkouts
                let db_path = git.join("db").join(ident.as_slice());
                let checkout_path = git.join("checkouts").join(ident.as_slice()).join(reference.as_slice());
                Ok(box GitSource::new(remote, reference.clone(), db_path, checkout_path) as Box<Source>)
            },
            ref PathKind => fail!("Cannot occur")
        }
    })));
     */

    pub fn load(&self, config: &Config) -> Box<Source> {
        match self.kind {
            GitKind(ref reference) => {
                box GitSource::new(self, config) as Box<Source>
            },
            PathKind => box PathSource::new(self) as Box<Source>,
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
    fn update(&self) -> CargoResult<()> {
        for source in self.sources.iter() {
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
}
