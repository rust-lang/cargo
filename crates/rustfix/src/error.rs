//! Error types.

use std::ops::Range;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid range {0:?}, start is larger than end")]
    InvalidRange(Range<usize>),

    #[error("invalid range {0:?}, original data is only {1} byte long")]
    DataLengthExceeded(Range<usize>, usize),

    #[error("could not replace range {0:?}, maybe parts of it were already replaced?")]
    MaybeAlreadyReplaced(Range<usize>),

    #[error("cannot replace slice of data that was already replaced")]
    AlreadyReplaced,

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}
