use std::fmt;

/// Types of the output artifact that the compiler emits.
/// Usually distributable or linkable either statically or dynamically.
///
/// See <https://doc.rust-lang.org/nightly/reference/linkage.html>.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CrateType {
    Bin,
    Lib,
    Rlib,
    Dylib,
    Cdylib,
    Staticlib,
    ProcMacro,
    Other(String),
}

impl CrateType {
    pub fn as_str(&self) -> &str {
        match self {
            CrateType::Bin => "bin",
            CrateType::Lib => "lib",
            CrateType::Rlib => "rlib",
            CrateType::Dylib => "dylib",
            CrateType::Cdylib => "cdylib",
            CrateType::Staticlib => "staticlib",
            CrateType::ProcMacro => "proc-macro",
            CrateType::Other(s) => s,
        }
    }
}

impl fmt::Display for CrateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl<'a> From<&'a String> for CrateType {
    fn from(s: &'a String) -> Self {
        match s.as_str() {
            "bin" => CrateType::Bin,
            "lib" => CrateType::Lib,
            "rlib" => CrateType::Rlib,
            "dylib" => CrateType::Dylib,
            "cdylib" => CrateType::Cdylib,
            "staticlib" => CrateType::Staticlib,
            "proc-macro" => CrateType::ProcMacro,
            _ => CrateType::Other(s.clone()),
        }
    }
}

impl From<String> for CrateType {
    fn from(s: String) -> Self {
        CrateType::from(&s)
    }
}

impl fmt::Debug for CrateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl serde::Serialize for CrateType {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        s.serialize_str(self.as_str())
    }
}
