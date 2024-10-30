//! Error types.

use std::ops::Range;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid range {0:?}, start is larger than end")]
    InvalidRange(Range<usize>),

    #[error("invalid range {0:?}, original data is only {1} byte long")]
    DataLengthExceeded(Range<usize>, usize),

    #[non_exhaustive] // There are plans to add fields to this variant at a later time.
    #[error("cannot replace slice of data that was already replaced")]
    AlreadyReplaced,

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}
