//! Error types.

use std::ops::Range;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid range {0:?}, start is larger than end")]
    InvalidRange(Range<usize>),

    #[error("invalid range {0:?}, original data is only {1} byte long")]
    DataLengthExceeded(Range<usize>, usize),

    #[non_exhaustive]
    #[error("cannot replace slice of data that was already replaced")]
    AlreadyReplaced {
        /// The location of the intended replacement.
        range: Range<usize>,
        /// Whether the modification exactly matches (both range and data) the one it conflicts with.
        /// Some clients may wish to simply ignore this condition.
        is_identical: bool,
    },

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}
