#[crate_type="rlib"];

extern crate serialize;
use serialize::{Decoder};

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Manifest {
  project: ~Project,
  root: ~str,
  lib: ~[LibTarget],
  bin: ~[ExecTarget]
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct ExecTarget {
  name: ~str,
  path: ~str
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct LibTarget {
  name: ~str,
  path: ~str
}

//pub type LibTarget = Target;
//pub type ExecTarget = Target;

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Project {
  name: ~str,
  version: ~str,
  authors: ~[~str]
}

