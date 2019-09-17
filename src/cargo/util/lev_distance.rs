use std::cmp;

pub fn lev_distance(me: &str, t: &str) -> usize {
    if me.is_empty() {
        return t.chars().count();
    }
    if t.is_empty() {
        return me.chars().count();
    }

    let mut dcol = (0..=t.len()).collect::<Vec<_>>();
    let mut t_last = 0;

    for (i, sc) in me.chars().enumerate() {
        let mut current = i;
        dcol[0] = current + 1;

        for (j, tc) in t.chars().enumerate() {
            let next = dcol[j + 1];

            if sc == tc {
                dcol[j + 1] = current;
            } else {
                dcol[j + 1] = cmp::min(current, next);
                dcol[j + 1] = cmp::min(dcol[j + 1], dcol[j]) + 1;
            }

            current = next;
            t_last = j;
        }
    }

    dcol[t_last + 1]
}

/// Find the closest element from `iter` matching `choice`. The `key` callback
/// is used to select a `&str` from the iterator to compare against `choice`.
pub fn closest<'a, T>(
    choice: &str,
    iter: impl Iterator<Item = T>,
    key: impl Fn(&T) -> &'a str,
) -> Option<T> {
    // Only consider candidates with a lev_distance of 3 or less so we don't
    // suggest out-of-the-blue options.
    iter.map(|e| (lev_distance(choice, key(&e)), e))
        .filter(|&(d, _)| d < 4)
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
fn test_lev_distance() {
    use std::char::{from_u32, MAX};
    // Test bytelength agnosticity
    for c in (0u32..MAX as u32)
        .filter_map(from_u32)
        .map(|i| i.to_string())
    {
        assert_eq!(lev_distance(&c, &c), 0);
    }

    let a = "\nMäry häd ä little lämb\n\nLittle lämb\n";
    let b = "\nMary häd ä little lämb\n\nLittle lämb\n";
    let c = "Mary häd ä little lämb\n\nLittle lämb\n";
    assert_eq!(lev_distance(a, b), 1);
    assert_eq!(lev_distance(b, a), 1);
    assert_eq!(lev_distance(a, c), 2);
    assert_eq!(lev_distance(c, a), 2);
    assert_eq!(lev_distance(b, c), 1);
    assert_eq!(lev_distance(c, b), 1);
}
