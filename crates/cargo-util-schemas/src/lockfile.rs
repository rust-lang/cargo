#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct EncodableSourceIdError(#[from] pub EncodableSourceIdErrorKind);

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum EncodableSourceIdErrorKind {
    #[error("invalid source `{0}`")]
    InvalidSource(String),

    #[error("invalid url `{url}`: {msg}")]
    InvalidUrl { url: String, msg: String },

    #[error("unsupported source protocol: {0}")]
    UnsupportedSource(String),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct EncodablePackageIdError(#[from] EncodablePackageIdErrorKind);

impl From<EncodableSourceIdError> for EncodablePackageIdError {
    fn from(value: EncodableSourceIdError) -> Self {
        EncodablePackageIdErrorKind::Source(value).into()
    }
}

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum EncodablePackageIdErrorKind {
    #[error("invalid serialied PackageId")]
    InvalidSerializedPackageId,

    #[error(transparent)]
    Source(#[from] EncodableSourceIdError),
}
