use std::error::Error as StdError;

use serde::{Deserialize, Serialize};

type BoxError = Box<dyn StdError + Sync + Send>;

/// Credential provider error type.
///
/// `UrlNotSupported` and `NotFound` errors both cause Cargo
/// to attempt another provider, if one is available. The other
/// variants are fatal.
///
/// Note: Do not add a tuple variant, as it cannot be serialized.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct Error {
    kind: ErrorKind,

    #[serde(flatten)]
    inner: Option<SerdeBoxError>,
}

impl Error {
    pub fn other(inner: BoxError) -> Self {
        Self {
            kind: ErrorKind::Other,
            inner: Some(SerdeBoxError(inner)),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.as_ref().and_then(|e| e.source())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(inner) = self.inner.as_ref() {
            inner.fmt(f)
        } else {
            self.kind.fmt(f)
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            kind: kind,
            inner: None,
        }
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Box::new(StringTypedError {
            message,
            source: None,
        })
        .into()
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        err.to_string().into()
    }
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Error::from(Box::new(StringTypedError::from(value)))
    }
}

impl<T: StdError + Send + Sync + 'static> From<Box<T>> for Error {
    fn from(value: Box<T>) -> Self {
        Error::other(value)
    }
}

/// Credential provider error kind.
///
/// `UrlNotSupported` and `NotFound` errors both cause Cargo
/// to attempt another provider, if one is available. The other
/// variants are fatal.
///
/// Note: Do not add a tuple variant, as it cannot be serialized.
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ErrorKind {
    /// Registry URL is not supported. This should be used if
    /// the provider only works for some registries. Cargo will
    /// try another provider, if available
    UrlNotSupported,

    /// Credentials could not be found. Cargo will try another
    /// provider, if available
    NotFound,

    /// The provider doesn't support this operation, such as
    /// a provider that can't support 'login' / 'logout'
    OperationNotSupported,

    /// The provider failed to perform the operation. Other
    /// providers will not be attempted
    Other,

    /// A new variant was added to this enum since Cargo was built
    #[serde(other)]
    Unknown,
}

impl StdError for ErrorKind {}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UrlNotSupported => f.write_str("registry not supported"),
            Self::NotFound => f.write_str("credential not found"),
            Self::OperationNotSupported => f.write_str("requested operation not supported"),
            Self::Other => f.write_str("credential action failed"),
            Self::Unknown => f.write_str("unknown error kind; try updating Cargo?"),
        }
    }
}

/// String-based error type with an optional source
#[derive(Debug)]
struct StringTypedError {
    message: String,
    source: Option<Box<StringTypedError>>,
}

impl StdError for StringTypedError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|err| err as &dyn StdError)
    }
}

impl std::fmt::Display for StringTypedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl From<anyhow::Error> for StringTypedError {
    fn from(value: anyhow::Error) -> Self {
        let mut prev = None;
        for e in value.chain().rev() {
            prev = Some(StringTypedError {
                message: e.to_string(),
                source: prev.map(Box::new),
            });
        }
        prev.unwrap()
    }
}

#[derive(Debug)]
struct SerdeBoxError(BoxError);

impl Serialize for SerdeBoxError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        use std::ops::Deref as _;

        let mut state = serializer.serialize_struct("StringTypedError", 2)?;
        state.serialize_field("message", &format!("{}", self.0))?;

        // Serialize the source error chain recursively
        let mut current_source: &dyn StdError = self.0.deref();
        let mut sources = Vec::new();
        while let Some(err) = current_source.source() {
            sources.push(err.to_string());
            current_source = err;
        }
        state.serialize_field("caused-by", &sources)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SerdeBoxError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = error_serialize::ErrorData::deserialize(deserializer)?;
        let e = SerdeBoxError(Box::new(StringTypedError::from(data)));
        Ok(e)
    }
}

impl StdError for SerdeBoxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl std::fmt::Display for SerdeBoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Serializer / deserializer for any boxed error.
/// The string representation of the error, and its `source` chain can roundtrip across
/// the serialization. The actual types are lost (downcast will not work).
mod error_serialize {
    use serde::Deserialize;

    use super::StringTypedError;

    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct ErrorData {
        message: String,
        caused_by: Option<Vec<String>>,
    }

    impl From<ErrorData> for StringTypedError {
        fn from(data: ErrorData) -> Self {
            let mut prev = None;
            if let Some(source) = data.caused_by {
                for e in source.into_iter().rev() {
                    prev = Some(Box::new(StringTypedError {
                        message: e,
                        source: prev,
                    }));
                }
            }
            StringTypedError {
                message: data.message,
                source: prev,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use super::ErrorKind;

    #[test]
    fn not_supported_roundtrip() {
        let input = Error::from(ErrorKind::UrlNotSupported);

        let expected_json = r#"{"kind":"url-not-supported"}"#;
        let actual_json = serde_json::to_string(&input).unwrap();
        assert_eq!(actual_json, expected_json);

        let actual: Error = serde_json::from_str(&actual_json).unwrap();
        assert!(matches!(actual.kind(), ErrorKind::UrlNotSupported));
    }

    #[test]
    fn deserialize_to_unknown_kind() {
        let json = r#"{
            "kind": "unexpected-kind",
            "unexpected-content": "test"
          }"#;
        let e: Error = serde_json::from_str(&json).unwrap();
        assert!(matches!(e.kind(), ErrorKind::Unknown));
    }

    #[test]
    fn other_roundtrip() {
        // Construct an error with context
        let e = anyhow::anyhow!("E1").context("E2").context("E3");
        // Convert to a string with contexts.
        let s1 = format!("{:?}", e);
        // Convert the error into an `Error`
        let e: Error = e.into();
        // Convert that error into JSON
        let json = serde_json::to_string_pretty(&e).unwrap();
        // Convert that error back to anyhow
        let e: anyhow::Error = e.into();
        let s2 = format!("{:?}", e);
        assert_eq!(s1, s2);

        // Convert the error back from JSON
        let e: Error = serde_json::from_str(&json).unwrap();
        // Convert to back to anyhow
        let e: anyhow::Error = e.into();
        let s3 = format!("{:?}", e);
        assert_eq!(s2, s3);

        assert_eq!(
            r#"{
  "kind": "other",
  "message": "E3",
  "caused-by": [
    "E2",
    "E1"
  ]
}"#,
            json
        );
    }
}
