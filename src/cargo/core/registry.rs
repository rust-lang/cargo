use std::vec::Vec;

use core::{
    Summary
};

pub trait Registry {
    fn query<'a>(&'a self, name: &str) -> Vec<&'a Summary>;
}

impl Registry for Vec<Summary> {
    fn query<'a>(&'a self, name: &str) -> Vec<&'a Summary> {
        self.iter()
          .filter(|summary| name == summary.get_name())
          .collect()
    }
}
