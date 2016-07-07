#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate serde_json;

pub mod diagnostics;

#[cfg(test)]
mod tests;
