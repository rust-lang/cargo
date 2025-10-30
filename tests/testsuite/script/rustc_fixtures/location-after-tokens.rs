#![feature(frontmatter)]

---
//~^ ERROR: expected item, found `-`
// FIXME(frontmatter): make this diagnostic better
---

// frontmatters must be at the start of a file. This test ensures that.
// CARGO(pass): not technitcally a frontmatter, so defer to rustc to error

fn main() {}
