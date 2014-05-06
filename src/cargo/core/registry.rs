use std::vec::Vec;

use core::{
  // Dependency,
  Package
};

pub trait Registry {
  fn query<'a>(&'a self, name: &str) -> Vec<&'a Package>;
}
