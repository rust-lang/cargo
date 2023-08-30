use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use thiserror::Error as ThisError;

/// Credential provider error type.
///
/// `UrlNotSupported` and `NotFound` errors both cause Cargo
/// to attempt another provider, if one is available. The other
/// variants are fatal.
///
/// Note: Do not add a tuple variant, as it cannot be serialized.
#[derive(Serialize, Deserialize, ThisError, Debug)]
#[serde(rename_all = "kebab-case", tag = "kind")]
#[non_exhaustive]
pub enum Error {
    /// Registry URL is not supported. This should be used if
    /// the provider only works for some registries. Cargo will
    /// try another provider, if available
    #[error("registry not supported")]
    UrlNotSupported,

    /// Credentials could not be found. Cargo will try another
    /// provider, if available
    #[error("credential not found")]
    NotFound,

    /// The provider doesn't support this operation, such as
    /// a provider that can't support 'login' / 'logout'
    #[error("requested operation not supported")]
    OperationNotSupported,

    /// The provider failed to perform the operation. Other
    /// providers will not be attempted
    #[error(transparent)]
    #[serde(with = "error_serialize")]
    Other(Box<dyn StdError + Sync + Send>),

    /// A new variant was added to this enum since Cargo was built
    #[error("unknown error kind; try updating Cargo?")]
    #[serde(other)]
    Unknown,
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
        let mut prev = None;
        for e in value.chain().rev() {
            prev = Some(Box::new(StringTypedError {
                message: e.to_string(),
                source: prev,
            }));
        }
        Error::Other(prev.unwrap())
    }
}

impl<T: StdError + Send + Sync + 'static> From<Box<T>> for Error {
    fn from(value: Box<T>) -> Self {
        Error::Other(value)
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

/// Serializer / deserializer for any boxed error.
/// The string representation of the error, and its `source` chain can roundtrip across
/// the serialization. The actual types are lost (downcast will not work).
mod error_serialize {
    use std::error::Error as StdError;
    use std::ops::Deref;

    use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serializer};

    use crate::error::StringTypedError;

    pub fn serialize<S>(
        e: &Box<dyn StdError + Send + Sync>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("StringTypedError", 2)?;
        state.serialize_field("message", &format!("{}", e))?;

        // Serialize the source error chain recursively
        let mut current_source: &dyn StdError = e.deref();
        let mut sources = Vec::new();
        while let Some(err) = current_source.source() {
            sources.push(err.to_string());
            current_source = err;
        }
        state.serialize_field("caused-by", &sources)?;
        state.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Box<dyn StdError + Sync + Send>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct ErrorData {
            message: String,
            caused_by: Option<Vec<String>>,
        }
        let data = ErrorData::deserialize(deserializer)?;
        let mut prev = None;
        if let Some(source) = data.caused_by {
            for e in source.into_iter().rev() {
                prev = Some(Box::new(StringTypedError {
                    message: e,
                    source: prev,
                }));
            }
        }
        let e = Box::new(StringTypedError {
            message: data.message,
            source: prev,
        });
        Ok(e)
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    pub fn unknown_kind() {
        let json = r#"{
            "kind": "unexpected-kind",
            "unexpected-content": "test"
          }"#;
        let e: Error = serde_json::from_str(&json).unwrap();
        assert!(matches!(e, Error::Unknown));
    }

    #[test]
    pub fn roundtrip() {
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
