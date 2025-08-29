  ---
//~^ ERROR: invalid preceding whitespace for frontmatter opening
//~^^ ERROR: unclosed frontmatter
  ---

#![feature(frontmatter)]

// check that whitespaces should not precede the frontmatter opening or close.
// CARGO(pass): not technitcally a frontmatter, so defer to rustc to error

fn main() {}
