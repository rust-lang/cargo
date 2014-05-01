use std::vec::Vec;

use core::{
  // Dependency,
  Package
};

pub trait Registry {
  fn query<'a>(&'a self, name: &str) -> Vec<&'a Package>;
}

/*
 *
 * ===== Temporary for convenience =====
 *
 */

/*
pub struct MemRegistry {
  packages: Vec<Package>
}

impl MemRegistry {
  pub fn new(packages: &Vec<Package>) -> MemRegistry {
    MemRegistry { packages: packages.clone() }
  }

  pub fn empty() -> MemRegistry {
    MemRegistry { packages: Vec::new() }
  }
}

impl Registry for MemRegistry {
  fn query<'a>(&'a self, name: &str) -> Vec<&'a Package> {
    self.packages.iter()
      .filter(|pkg| name == pkg.get_name())
      .collect()
  }
}
*/
