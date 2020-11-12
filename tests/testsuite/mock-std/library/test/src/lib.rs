#![feature(staged_api)]
#![feature(test)]
#![unstable(feature = "test", issue = "none")]

extern crate test;

pub use test::*;

pub fn custom_api() {
    registry_dep_only_used_by_test::wow_testing_is_so_easy();
}
