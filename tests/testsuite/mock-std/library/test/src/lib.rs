#![feature(staged_api)]
#![feature(test)]
#![unstable(feature = "test", issue = "none")]

extern crate test;

pub use test::*;

pub fn custom_api() {
}
