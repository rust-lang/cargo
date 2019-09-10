#![feature(test)]

extern crate test;

pub use test::*;

pub fn custom_api() {
    registry_dep_using_std::custom_api();
}
