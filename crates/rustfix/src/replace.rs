//! A small module giving you a simple container that allows easy and cheap
//! replacement of parts of its content, with the ability to prevent changing
//! the same parts multiple times.

use std::rc::Rc;

use crate::error::Error;

/// Indicates the change state of a [`Span`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    /// The initial state. No change applied.
    Initial,
    /// Has been replaced.
    Replaced(Rc<[u8]>),
    /// Has been inserted.
    Inserted(Rc<[u8]>),
}

impl State {
    fn is_inserted(&self) -> bool {
        matches!(*self, State::Inserted(..))
    }
}

/// Span with a change [`State`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    /// Start of this span in parent data
    start: usize,
    /// up to end excluding
    end: usize,
    /// Whether the span is inserted, replaced or still fresh.
    data: State,
}

/// A container that allows easily replacing chunks of its data
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// Original data.
    original: Vec<u8>,
    /// [`Span`]s covering the full range of the original data.
    parts: Vec<Span>,
}

impl Data {
    /// Create a new data container from a slice of bytes
    pub fn new(data: &[u8]) -> Self {
        Data {
            original: data.into(),
            parts: vec![Span {
                data: State::Initial,
                start: 0,
                end: data.len(),
            }],
        }
    }

    /// Render this data as a vector of bytes
    pub fn to_vec(&self) -> Vec<u8> {
        if self.original.is_empty() {
            return Vec::new();
        }

        self.parts.iter().fold(Vec::new(), |mut acc, d| {
            match d.data {
                State::Initial => acc.extend_from_slice(&self.original[d.start..d.end]),
                State::Replaced(ref d) | State::Inserted(ref d) => acc.extend_from_slice(d),
            };
            acc
        })
    }

    /// Replace a chunk of data with the given slice, erroring when this part
    /// was already changed previously.
    pub fn replace_range(
        &mut self,
        range: std::ops::Range<usize>,
        data: &[u8],
    ) -> Result<(), Error> {
        if range.start > range.end {
            return Err(Error::InvalidRange(range));
        }

        if range.end > self.original.len() {
            return Err(Error::DataLengthExceeded(range, self.original.len()));
        }

        let insert_only = range.start == range.end;

        // Since we error out when replacing an already replaced chunk of data,
        // we can take some shortcuts here. For example, there can be no
        // overlapping replacements -- we _always_ split a chunk of 'initial'
        // data into three[^empty] parts, and there can't ever be two 'initial'
        // parts touching.
        //
        // [^empty]: Leading and trailing ones might be empty if we replace
        // the whole chunk. As an optimization and without loss of generality we
        // don't add empty parts.
        let new_parts = {
            let Some(index_of_part_to_split) = self.parts.iter().position(|p| {
                !p.data.is_inserted() && p.start <= range.start && p.end >= range.end
            }) else {
                if tracing::enabled!(tracing::Level::DEBUG) {
                    let slices = self
                        .parts
                        .iter()
                        .map(|p| {
                            (
                                p.start,
                                p.end,
                                match p.data {
                                    State::Initial => "initial",
                                    State::Replaced(..) => "replaced",
                                    State::Inserted(..) => "inserted",
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    tracing::debug!(
                        "no single slice covering {}..{}, current slices: {:?}",
                        range.start,
                        range.end,
                        slices,
                    );
                }

                return Err(Error::MaybeAlreadyReplaced(range));
            };

            let part_to_split = &self.parts[index_of_part_to_split];

            // If this replacement matches exactly the part that we would
            // otherwise split then we ignore this for now. This means that you
            // can replace the exact same range with the exact same content
            // multiple times and we'll process and allow it.
            //
            // This is currently done to alleviate issues like
            // rust-lang/rust#51211 although this clause likely wants to be
            // removed if that's fixed deeper in the compiler.
            if part_to_split.start == range.start && part_to_split.end == range.end {
                if let State::Replaced(ref replacement) = part_to_split.data {
                    if &**replacement == data {
                        return Ok(());
                    }
                }
            }

            if part_to_split.data != State::Initial {
                return Err(Error::AlreadyReplaced);
            }

            let mut new_parts = Vec::with_capacity(self.parts.len() + 2);

            // Previous parts
            if let Some(ps) = self.parts.get(..index_of_part_to_split) {
                new_parts.extend_from_slice(ps);
            }

            // Keep initial data on left side of part
            if range.start > part_to_split.start {
                new_parts.push(Span {
                    start: part_to_split.start,
                    end: range.start,
                    data: State::Initial,
                });
            }

            // New part
            new_parts.push(Span {
                start: range.start,
                end: range.end,
                data: if insert_only {
                    State::Inserted(data.into())
                } else {
                    State::Replaced(data.into())
                },
            });

            // Keep initial data on right side of part
            if range.end < part_to_split.end {
                new_parts.push(Span {
                    start: range.end,
                    end: part_to_split.end,
                    data: State::Initial,
                });
            }

            // Following parts
            if let Some(ps) = self.parts.get(index_of_part_to_split + 1..) {
                new_parts.extend_from_slice(ps);
            }

            new_parts
        };

        self.parts = new_parts;

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
    fn replace_overlapping_stuff_errs() {
        let mut d = Data::new(b"foo bar baz");

        d.replace_range(4..7, b"lol").unwrap();
        assert_eq!("foo lol baz", str(&d.to_vec()));

        assert!(matches!(
            d.replace_range(4..7, b"lol2").unwrap_err(),
            Error::AlreadyReplaced,
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
    fn replace_same_twice() {
        let mut d = Data::new(b"foo");
        d.replace_range(0..1, b"b").unwrap();
        d.replace_range(0..1, b"b").unwrap();
        assert_eq!("boo", str(&d.to_vec()));
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
