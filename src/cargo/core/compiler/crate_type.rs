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

    /// Returns whether production of this crate type could benefit from splitting metadata
    /// into a .rmeta file.
    ///
    /// See also [`TargetKind::benefits_from_no_embed_metadata`].
    ///
    /// [`TargetKind::benefits_from_no_embed_metadata`]: crate::core::manifest::TargetKind::benefits_from_no_embed_metadata
    pub fn benefits_from_no_embed_metadata(&self) -> bool {
        match self {
            // rlib/libs generate .rmeta files for pipelined compilation.
            // If we also include metadata inside of them, we waste disk space, since the metadata
            // will be located both in the lib/rlib and the .rmeta file.
            CrateType::Lib |
            CrateType::Rlib |
            // Dylibs do not have to contain metadata when they are used as a runtime dependency.
            // If we split the metadata into a separate .rmeta file, the dylib file (that
            // can be shipped as a runtime dependency) can be smaller.
            CrateType::Dylib => true,
            // Proc macros contain metadata that specifies what macro functions are available in
            // it, but the metadata is typically very small. The metadata of proc macros is also
            // self-contained (unlike rlibs/dylibs), so let's not unnecessarily split it into
            // multiple files.
            CrateType::ProcMacro |
            // cdylib and staticlib produce artifacts that are used through the C ABI and do not
            // contain Rust-specific metadata.
            CrateType::Cdylib |
            CrateType::Staticlib |
            // Binaries also do not contain metadata
            CrateType::Bin |
            CrateType::Other(_) => false
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
