use std::vec::Vec;
use core;

/**
 * Represents a rust library internally to cargo. This will things like where
 * on the local system the code is located, it's remote location, dependencies,
 * etc..
 *
 * This differs from core::Project
 */
#[deriving(Clone,Eq,Show)]
pub struct Package {
  name: ~str,
  deps: Vec<core::Dependency>
}

impl Package {
  pub fn new(name: &str, deps: &Vec<core::Dependency>) -> Package {
    Package { name: name.to_owned(), deps: deps.clone() }
  }

  pub fn get_name<'a>(&'a self) -> &'a str {
    self.name.as_slice()
  }

  pub fn get_dependencies<'a>(&'a self) -> &'a Vec<core::Dependency> {
      &self.deps
  }
}
