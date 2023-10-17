use std::{cmp, mem};

/// Finds the [edit distance] between two strings.
///
/// Returns `None` if the distance exceeds the limit.
///
/// [edit distance]: https://en.wikipedia.org/wiki/Edit_distance
pub fn edit_distance(a: &str, b: &str, limit: usize) -> Option<usize> {
    // Comparing the strings lowercased will result in a difference in capitalization being less distance away
    // than being a completely different letter. Otherwise `CHECK` is as far away from `check` as it
    // is from `build` (both with a distance of 5). For a single letter shortcut (e.g. `b` or `c`), they will
    // all be as far away from any capital single letter entry (all with a distance of 1).
    // By first lowercasing the strings, `C` and `c` are closer than `C` and `b`, for example.
    let a = a.to_lowercase();
    let b = b.to_lowercase();

    let mut a = &a.chars().collect::<Vec<_>>()[..];
    let mut b = &b.chars().collect::<Vec<_>>()[..];

    // Ensure that `b` is the shorter string, minimizing memory use.
    if a.len() < b.len() {
        mem::swap(&mut a, &mut b);
    }

    let min_dist = a.len() - b.len();
    // If we know the limit will be exceeded, we can return early.
    if min_dist > limit {
        return None;
    }

    // Strip common prefix.
    while let Some(((b_char, b_rest), (a_char, a_rest))) = b.split_first().zip(a.split_first()) {
        if a_char != b_char {
            break;
        }
        a = a_rest;
        b = b_rest;
    }
    // Strip common suffix.
    while let Some(((b_char, b_rest), (a_char, a_rest))) = b.split_last().zip(a.split_last()) {
        if a_char != b_char {
            break;
        }
        a = a_rest;
        b = b_rest;
    }

    // If either string is empty, the distance is the length of the other.
    // We know that `b` is the shorter string, so we don't need to check `a`.
    if b.len() == 0 {
        return Some(min_dist);
    }

    let mut prev_prev = vec![usize::MAX; b.len() + 1];
    let mut prev = (0..=b.len()).collect::<Vec<_>>();
    let mut current = vec![0; b.len() + 1];

    // row by row
    for i in 1..=a.len() {
        current[0] = i;
        let a_idx = i - 1;

        // column by column
        for j in 1..=b.len() {
            let b_idx = j - 1;

            // There is no cost to substitute a character with itself.
            let substitution_cost = if a[a_idx] == b[b_idx] { 0 } else { 1 };

            current[j] = cmp::min(
                // deletion
                prev[j] + 1,
                cmp::min(
                    // insertion
                    current[j - 1] + 1,
                    // substitution
                    prev[j - 1] + substitution_cost,
                ),
            );

            if (i > 1) && (j > 1) && (a[a_idx] == b[b_idx - 1]) && (a[a_idx - 1] == b[b_idx]) {
                // transposition
                current[j] = cmp::min(current[j], prev_prev[j - 2] + 1);
            }
        }

        // Rotate the buffers, reusing the memory.
        [prev_prev, prev, current] = [prev, current, prev_prev];
    }

    // `prev` because we already rotated the buffers.
    let distance = prev[b.len()];
    (distance <= limit).then_some(distance)
}

/// Find the closest element from `iter` matching `choice`. The `key` callback
/// is used to select a `&str` from the iterator to compare against `choice`.
pub fn closest<'a, T>(
    choice: &str,
    iter: impl Iterator<Item = T>,
    key: impl Fn(&T) -> &'a str,
) -> Option<T> {
    // Only consider candidates with an edit distance of 3 or less so we don't
    // suggest out-of-the-blue options.
    iter.filter_map(|e| Some((edit_distance(choice, key(&e), 3)?, e)))
        .min_by_key(|t| t.0)
        .map(|t| t.1)
}

/// Version of `closest` that returns a common "suggestion" that can be tacked
/// onto the end of an error message.
pub fn closest_msg<'a, T>(
    choice: &str,
    iter: impl Iterator<Item = T>,
    key: impl Fn(&T) -> &'a str,
) -> String {
    match closest(choice, iter, &key) {
        Some(e) => format!("\n\n\tDid you mean `{}`?", key(&e)),
        None => String::new(),
    }
}

#[test]
fn test_edit_distance() {
    use std::char::{from_u32, MAX};
    // Test bytelength agnosticity
    for c in (0u32..MAX as u32)
        .filter_map(from_u32)
        .map(|i| i.to_string())
    {
        assert_eq!(edit_distance(&c, &c, usize::MAX), Some(0));
    }

    let a = "\nMäry häd ä little lämb\n\nLittle lämb\n";
    let b = "\nMary häd ä little lämb\n\nLittle lämb\n";
    let c = "Mary häd ä little lämb\n\nLittle lämb\n";
    assert_eq!(edit_distance(a, b, usize::MAX), Some(1));
    assert_eq!(edit_distance(b, a, usize::MAX), Some(1));
    assert_eq!(edit_distance(a, c, usize::MAX), Some(2));
    assert_eq!(edit_distance(c, a, usize::MAX), Some(2));
    assert_eq!(edit_distance(b, c, usize::MAX), Some(1));
    assert_eq!(edit_distance(c, b, usize::MAX), Some(1));
}
