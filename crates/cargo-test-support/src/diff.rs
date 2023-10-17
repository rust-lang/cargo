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

pub fn diff<'a, T>(a: &'a [T], b: &'a [T]) -> Vec<Change<&'a T>>
where
    T: PartialEq,
{
    if a.is_empty() && b.is_empty() {
        return vec![];
    }
    let mut diff = vec![];
    for (prev_x, prev_y, x, y) in backtrack(&a, &b) {
        if x == prev_x {
            diff.push(Change::Add(prev_y + 1, &b[prev_y]));
        } else if y == prev_y {
            diff.push(Change::Remove(prev_x + 1, &a[prev_x]));
        } else {
            diff.push(Change::Keep(prev_x + 1, prev_y + 1, &a[prev_x]));
        }
    }
    diff.reverse();
    diff
}

fn shortest_edit<T>(a: &[T], b: &[T]) -> Vec<Vec<usize>>
where
    T: PartialEq,
{
    let max = a.len() + b.len();
    let mut v = vec![0; 2 * max + 1];
    let mut trace = vec![];
    for d in 0..=max {
        trace.push(v.clone());
        for k in (0..=(2 * d)).step_by(2) {
            let mut x = if k == 0 || (k != 2 * d && v[max - d + k - 1] < v[max - d + k + 1]) {
                // Move down
                v[max - d + k + 1]
            } else {
                // Move right
                v[max - d + k - 1] + 1
            };
            let mut y = x + d - k;
            // Step diagonally as far as possible.
            while x < a.len() && y < b.len() && a[x] == b[y] {
                x += 1;
                y += 1;
            }
            v[max - d + k] = x;
            // Return if reached the bottom-right position.
            if x >= a.len() && y >= b.len() {
                return trace;
            }
        }
    }
    panic!("finished without hitting end?");
}

fn backtrack<T>(a: &[T], b: &[T]) -> Vec<(usize, usize, usize, usize)>
where
    T: PartialEq,
{
    let mut result = vec![];
    let mut x = a.len();
    let mut y = b.len();
    let max = x + y;
    for (d, v) in shortest_edit(a, b).iter().enumerate().rev() {
        let k = x + d - y;
        let prev_k = if k == 0 || (k != 2 * d && v[max - d + k - 1] < v[max - d + k + 1]) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[max - d + prev_k];
        let prev_y = (prev_x + d).saturating_sub(prev_k);
        while x > prev_x && y > prev_y {
            result.push((x - 1, y - 1, x, y));
            x -= 1;
            y -= 1;
        }
        if d > 0 {
            result.push((prev_x, prev_y, x, y));
        }
        x = prev_x;
        y = prev_y;
    }
    return result;
}

pub fn colored_diff<'a, T>(a: &'a [T], b: &'a [T]) -> String
where
    T: PartialEq + fmt::Display,
{
    let changes = diff(a, b);
    render_colored_changes(&changes)
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

#[cfg(test)]
pub fn compare(a: &str, b: &str) {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    let changes = diff(&a, &b);
    let mut result = vec![];
    for change in changes {
        match change {
            Change::Add(_, s) => result.push(*s),
            Change::Remove(_, _s) => {}
            Change::Keep(_, _, s) => result.push(*s),
        }
    }
    assert_eq!(b, result);
}

#[test]
fn basic_tests() {
    compare("", "");
    compare("A", "");
    compare("", "B");
    compare("ABCABBA", "CBABAC");
}
