#![feature(staged_api)]
#![feature(test)]
#![unstable(feature = "test", issue = "0")]

extern crate test;

pub use test::*;

pub fn custom_api() {
}
