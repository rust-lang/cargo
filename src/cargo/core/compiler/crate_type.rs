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

    pub fn can_lto(&self) -> bool {
        match self {
            CrateType::Bin | CrateType::Staticlib | CrateType::Cdylib => true,
            CrateType::Lib
            | CrateType::Rlib
            | CrateType::Dylib
            | CrateType::ProcMacro
            | CrateType::Other(..) => false,
        }
    }

    pub fn is_linkable(&self) -> bool {
        match self {
            CrateType::Lib | CrateType::Rlib | CrateType::Dylib | CrateType::ProcMacro => true,
            CrateType::Bin | CrateType::Cdylib | CrateType::Staticlib | CrateType::Other(..) => {
                false
            }
        }
    }

    pub fn is_dynamic(&self) -> bool {
        match self {
            CrateType::Dylib | CrateType::Cdylib | CrateType::ProcMacro => true,
            CrateType::Lib
            | CrateType::Rlib
            | CrateType::Bin
            | CrateType::Staticlib
            | CrateType::Other(..) => false,
        }
    }

    /// Returns whether production of this crate type requires the object files
    /// from dependencies to be available.
    ///
    /// See also [`TargetKind::requires_upstream_objects`].
    ///
    /// [`TargetKind::requires_upstream_objects`]: crate::core::manifest::TargetKind::requires_upstream_objects
    pub fn requires_upstream_objects(&self) -> bool {
        // "lib" == "rlib" and is a compilation that doesn't actually
        // require upstream object files to exist, only upstream metadata
        // files. As a result, it doesn't require upstream artifacts

        !matches!(self, CrateType::Lib | CrateType::Rlib)
        // Everything else, however, is some form of "linkable output" or
        // something that requires upstream object files.
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
            "procmacro" => CrateType::ProcMacro,
            _ => CrateType::Other(s.clone()),
        }
    }
}

impl fmt::Debug for CrateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_string().fmt(f)
    }
}

impl serde::Serialize for CrateType {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        self.to_string().serialize(s)
    }
}
