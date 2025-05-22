//! A small module giving you a simple container that allows
//! easy and cheap replacement of parts of its content,
//! with the ability to commit or rollback pending changes.
//!
//! Create a new [`Data`] struct with some initial set of code,
//! then record changes with [`Data::replace_range`],
//! which will validate that the changes do not conflict with one another.
//! At any time, you can "checkpoint" the current changes with [`Data::commit`]
//! or roll them back (perhaps due to a conflict) with [`Data::restore`].
//! When you're done, use [`Data::to_vec`]
//! to merge the original data with the changes.
//!
//! # Notes
//!
//! The [`Data::to_vec`] method includes uncommitted changes, if present.
//! The reason for including uncommitted changes is that typically, once you're calling those,
//! you're done with edits and will be dropping the [`Data`] struct in a moment.
//! In this case, requiring an extra call to `commit` would be unnecessary work.
//! Of course, there's no harm in calling `commit`---it's just not strictly necessary.
//!
//! Put another way, the main point of `commit` is to checkpoint a set of known-good changes
//! before applying additional sets of as-of-yet unvalidated changes.
//! If no future changes are expected, you aren't _required_ to pay the cost of `commit`.
//! If you want to discard uncommitted changes, simply call [`Data::restore`] first.

use std::ops::Range;
use std::rc::Rc;

use crate::error::Error;

/// Data that should replace a particular range of the original.
#[derive(Clone)]
struct Span {
    /// Span of the parent data to be replaced, inclusive of the start, exclusive of the end.
    range: Range<usize>,
    /// New data to insert at the `start` position of the `original` data.
    data: Rc<[u8]>,
    /// Whether this data is committed or provisional.
    committed: bool,
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.is_insert() {
            "inserted"
        } else {
            "replaced"
        };

        let committed = if self.committed {
            "committed"
        } else {
            "uncommitted"
        };

        write!(
            f,
            "({}, {}: {state}, {committed})",
            self.range.start, self.range.end
        )
    }
}

impl Span {
    fn new(range: Range<usize>, data: &[u8]) -> Self {
        Self {
            range,
            data: data.into(),
            committed: false,
        }
    }

    /// Returns `true` if and only if this is a "pure" insertion,
    /// i.e. does not remove any existing data.
    ///
    /// The insertion point is the `start` position of the range.
    fn is_insert(&self) -> bool {
        self.range.start == self.range.end
    }
}

impl PartialEq for Span {
    /// Returns `true` if and only if this `Span` and `other` have the same range and data,
    /// regardless of `committed` status.
    fn eq(&self, other: &Self) -> bool {
        self.range == other.range && self.data == other.data
    }
}

/// A container that allows easily replacing chunks of its data.
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// Original data.
    original: Vec<u8>,
    /// [`Span`]s covering the full range of the original data.
    /// Important: it's expected that the underlying implementation maintains this in order,
    /// sorted ascending by start position.
    parts: Vec<Span>,
}

impl Data {
    /// Create a new data container from a slice of bytes
    pub fn new(data: &[u8]) -> Self {
        Data {
            original: data.into(),
            parts: vec![],
        }
    }

    /// Commit the current changes.
    pub fn commit(&mut self) {
        self.parts.iter_mut().for_each(|span| span.committed = true);
    }

    /// Discard uncommitted changes.
    pub fn restore(&mut self) {
        self.parts.retain(|parts| parts.committed);
    }

    /// Merge the original data with changes, **including** uncommitted changes.
    ///
    /// See the module-level documentation for more information on why uncommitted changes are included.
    pub fn to_vec(&self) -> Vec<u8> {
        let mut prev_end = 0;
        let mut s = self.parts.iter().fold(Vec::new(), |mut acc, span| {
            // Hedge against potential implementation errors.
            debug_assert!(
                prev_end <= span.range.start,
                "expected parts in sorted order"
            );

            acc.extend_from_slice(&self.original[prev_end..span.range.start]);
            acc.extend_from_slice(&span.data);
            prev_end = span.range.end;
            acc
        });

        // Append remaining data, if any.
        s.extend_from_slice(&self.original[prev_end..]);
        s
    }

    /// Record a provisional change.
    ///
    /// If committed, the original data in the given `range` will be replaced by the given data.
    /// If there already exist changes for data in the given range (committed or not),
    /// this method will return an error.
    /// It will also return an error if the beginning of the range comes before its end,
    /// or if the range is outside that of the original data.
    pub fn replace_range(&mut self, range: Range<usize>, data: &[u8]) -> Result<(), Error> {
        if range.start > range.end {
            return Err(Error::InvalidRange(range));
        }

        if range.end > self.original.len() {
            return Err(Error::DataLengthExceeded(range, self.original.len()));
        }

        // Keep sorted by start position, or by end position if the start position is the same,
        // which has the effect of keeping a pure insertion ahead of a replacement.
        // That limits the kinds of conflicts that can happen, simplifying the checks below.
        let ins_point = self.parts.partition_point(|span| {
            span.range.start < range.start
                || (span.range.start == range.start && span.range.end < range.end)
        });

        let incoming = Span::new(range, data.as_ref());

        // Reject if the change starts before the previous one ends.
        if let Some(before) = ins_point.checked_sub(1).and_then(|i| self.parts.get(i)) {
            if incoming.range.start < before.range.end {
                return Err(Error::AlreadyReplaced {
                    is_identical: incoming == *before,
                    range: incoming.range,
                });
            }
        }

        // Reject if the change ends after the next one starts,
        // or if this is an insert and there's already an insert there.
        if let Some(after) = self.parts.get(ins_point) {
            if incoming.range.end > after.range.start || incoming.range == after.range {
                return Err(Error::AlreadyReplaced {
                    is_identical: incoming == *after,
                    range: incoming.range,
                });
            }
        }

        self.parts.insert(ins_point, incoming);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn str(i: &[u8]) -> &str {
        ::std::str::from_utf8(i).unwrap()
    }

    #[test]
    fn insert_at_beginning() {
        let mut d = Data::new(b"foo bar baz");
        d.replace_range(0..0, b"oh no ").unwrap();
        assert_eq!("oh no foo bar baz", str(&d.to_vec()));
    }

    #[test]
    fn insert_at_end() {
        let mut d = Data::new(b"foo bar baz");
        d.replace_range(11..11, b" oh no").unwrap();
        assert_eq!("foo bar baz oh no", str(&d.to_vec()));
    }

    #[test]
    fn replace_some_stuff() {
        let mut d = Data::new(b"foo bar baz");
        d.replace_range(4..7, b"lol").unwrap();
        assert_eq!("foo lol baz", str(&d.to_vec()));
    }

    #[test]
    fn replace_a_single_char() {
        let mut d = Data::new(b"let y = true;");
        d.replace_range(4..5, b"mut y").unwrap();
        assert_eq!("let mut y = true;", str(&d.to_vec()));
    }

    #[test]
    fn replace_multiple_lines() {
        let mut d = Data::new(b"lorem\nipsum\ndolor");

        d.replace_range(6..11, b"lol").unwrap();
        assert_eq!("lorem\nlol\ndolor", str(&d.to_vec()));

        d.replace_range(12..17, b"lol").unwrap();
        assert_eq!("lorem\nlol\nlol", str(&d.to_vec()));
    }

    #[test]
    fn replace_multiple_lines_with_insert_only() {
        let mut d = Data::new(b"foo!");

        d.replace_range(3..3, b"bar").unwrap();
        assert_eq!("foobar!", str(&d.to_vec()));

        d.replace_range(0..3, b"baz").unwrap();
        assert_eq!("bazbar!", str(&d.to_vec()));

        d.replace_range(3..4, b"?").unwrap();
        assert_eq!("bazbar?", str(&d.to_vec()));
    }

    #[test]
    #[allow(clippy::reversed_empty_ranges)]
    fn replace_invalid_range() {
        let mut d = Data::new(b"foo!");

        assert!(d.replace_range(2..1, b"bar").is_err());
        assert!(d.replace_range(0..3, b"bar").is_ok());
    }

    #[test]
    fn empty_to_vec_roundtrip() {
        let s = "";
        assert_eq!(s.as_bytes(), Data::new(s.as_bytes()).to_vec().as_slice());
    }

    #[test]
    fn replace_same_range_diff_data() {
        let mut d = Data::new(b"foo bar baz");

        d.replace_range(4..7, b"lol").unwrap();
        assert_eq!("foo lol baz", str(&d.to_vec()));

        assert!(matches!(
            d.replace_range(4..7, b"lol2").unwrap_err(),
            Error::AlreadyReplaced {
                is_identical: false,
                ..
            },
        ));
    }

    #[test]
    fn replace_same_range_same_data() {
        let mut d = Data::new(b"foo bar baz");

        d.replace_range(4..7, b"lol").unwrap();
        assert_eq!("foo lol baz", str(&d.to_vec()));

        assert!(matches!(
            d.replace_range(4..7, b"lol").unwrap_err(),
            Error::AlreadyReplaced {
                is_identical: true,
                ..
            },
        ));
    }

    #[test]
    fn broken_replacements() {
        let mut d = Data::new(b"foo");
        assert!(matches!(
            d.replace_range(4..8, b"lol").unwrap_err(),
            Error::DataLengthExceeded(std::ops::Range { start: 4, end: 8 }, 3),
        ));
    }

    #[test]
    fn insert_same_twice() {
        let mut d = Data::new(b"foo");
        d.replace_range(1..1, b"b").unwrap();
        assert_eq!("fboo", str(&d.to_vec()));
        assert!(matches!(
            d.replace_range(1..1, b"b").unwrap_err(),
            Error::AlreadyReplaced {
                is_identical: true,
                ..
            },
        ));
        assert_eq!("fboo", str(&d.to_vec()));
    }

    #[test]
    fn commit_restore() {
        let mut d = Data::new(b", ");
        assert_eq!(", ", str(&d.to_vec()));

        d.replace_range(2..2, b"world").unwrap();
        d.replace_range(0..0, b"hello").unwrap();
        assert_eq!("hello, world", str(&d.to_vec()));

        d.restore();
        assert_eq!(", ", str(&d.to_vec()));

        d.commit();
        assert_eq!(", ", str(&d.to_vec()));

        d.replace_range(2..2, b"world").unwrap();
        assert_eq!(", world", str(&d.to_vec()));
        d.commit();
        assert_eq!(", world", str(&d.to_vec()));
        d.restore();
        assert_eq!(", world", str(&d.to_vec()));

        d.replace_range(0..0, b"hello").unwrap();
        assert_eq!("hello, world", str(&d.to_vec()));
        d.commit();
        assert_eq!("hello, world", str(&d.to_vec()));
        d.restore();
        assert_eq!("hello, world", str(&d.to_vec()));
    }

    proptest! {
        #[test]
        fn new_to_vec_roundtrip(ref s in "\\PC*") {
            assert_eq!(s.as_bytes(), Data::new(s.as_bytes()).to_vec().as_slice());
        }

        #[test]
        fn replace_random_chunks(
            ref data in "\\PC*",
            ref replacements in prop::collection::vec(
                (any::<::std::ops::Range<usize>>(), any::<Vec<u8>>()),
                1..100,
            )
        ) {
            let mut d = Data::new(data.as_bytes());
            for &(ref range, ref bytes) in replacements {
                let _ = d.replace_range(range.clone(), bytes);
            }
        }
    }
}
