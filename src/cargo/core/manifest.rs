
/*
 * TODO: Make all struct fields private
 */

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Manifest {
    pub project: ~Project,
    pub root: ~str,
    pub lib: ~[LibTarget],
    pub bin: ~[ExecTarget]
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
