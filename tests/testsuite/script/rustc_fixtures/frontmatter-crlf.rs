#!/usr/bin/env -S cargo -Zscript
---
[dependencies]
clap = "4"
---

//@ check-pass

// crlf line endings should be accepted

#![feature(frontmatter)]

fn main() {}
