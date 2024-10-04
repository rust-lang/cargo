//! A simple Myers diff implementation.
//!
//! This focuses on being short and simple, and the expense of being
//! inefficient. A key characteristic here is that this supports cargotest's
//! `[..]` wildcard matching. That means things like hashing can't be used.
//! Since Cargo's output tends to be small, this should be sufficient.

use std::fmt;
use std::io::Write;

/// A single line change to be applied to the original.
#[derive(Debug, Eq, PartialEq)]
pub enum Change<T> {
    Add(usize, T),
    Remove(usize, T),
    Keep(usize, usize, T),
}

pub fn render_colored_changes<T: fmt::Display>(changes: &[Change<T>]) -> String {
    // anstyle is not very ergonomic, but I don't want to bring in another dependency.
    let red = anstyle::AnsiColor::Red.on_default().render();
    let green = anstyle::AnsiColor::Green.on_default().render();
    let dim = (anstyle::Style::new() | anstyle::Effects::DIMMED).render();
    let bold = (anstyle::Style::new() | anstyle::Effects::BOLD).render();
    let reset = anstyle::Reset.render();

    let choice = if crate::is_ci() {
        // Don't use color on CI. Even though GitHub can display colors, it
        // makes reading the raw logs more difficult.
        anstream::ColorChoice::Never
    } else {
        anstream::AutoStream::choice(&std::io::stdout())
    };
    let mut buffer = anstream::AutoStream::new(Vec::new(), choice);

    for change in changes {
        let (nums, sign, color, text) = match change {
            Change::Add(i, s) => (format!("    {:<4} ", i), '+', green, s),
            Change::Remove(i, s) => (format!("{:<4}     ", i), '-', red, s),
            Change::Keep(x, y, s) => (format!("{:<4}{:<4} ", x, y), ' ', dim, s),
        };
        writeln!(
            buffer,
            "{dim}{nums}{reset}{bold}{sign}{reset}{color}{text}{reset}"
        )
        .unwrap();
    }
    String::from_utf8(buffer.into_inner()).unwrap()
}
