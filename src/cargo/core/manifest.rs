use core::package::NameVer;

/*
 * TODO: Make all struct fields private
 */

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Manifest {
    pub project: ~Project,
    pub root: ~str,
    pub lib: ~[LibTarget],
    pub bin: ~[ExecTarget],
    pub dependencies: Vec<NameVer>
}

impl Manifest {
    pub fn get_name_ver(&self) -> NameVer {
        NameVer::new(self.project.name.as_slice(), self.project.version.as_slice())
    }

    pub fn get_path<'a>(&'a self) -> Path {
        Path::new(self.root.as_slice())
    }
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct ExecTarget {
    pub name: ~str,
    pub path: ~str
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct LibTarget {
    pub name: ~str,
    pub path: ~str
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Project {
    pub name: ~str,
    pub version: ~str,
    pub authors: ~[~str]
}
