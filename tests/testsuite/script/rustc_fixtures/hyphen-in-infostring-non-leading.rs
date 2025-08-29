--- Cargo-toml
---

// infostrings can contain hyphens as long as a hyphen isn't the first character.
//@ check-pass
// CARGO(fail): unsupported infostring

#![feature(frontmatter)]

fn main() {}
