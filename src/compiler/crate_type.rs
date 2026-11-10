use cargo_util_schemas::core::CrateType;

/// Compiler-specific queries on [`CrateType`].
pub trait CrateTypeExt {
    fn can_lto(&self) -> bool;

    fn is_linkable(&self) -> bool;

    fn is_dynamic(&self) -> bool;

    /// Returns whether production of this crate type requires the object files
    /// from dependencies to be available.
    ///
    /// See also [`TargetKind::requires_upstream_objects`].
    ///
    /// [`TargetKind::requires_upstream_objects`]: crate::workspace::manifest::TargetKind::requires_upstream_objects
    fn requires_upstream_objects(&self) -> bool;

    /// Returns whether production of this crate type could benefit from splitting metadata
    /// into a .rmeta file.
    ///
    /// See also [`TargetKind::benefits_from_no_embed_metadata`].
    ///
    /// [`TargetKind::benefits_from_no_embed_metadata`]: crate::workspace::manifest::TargetKind::benefits_from_no_embed_metadata
    fn benefits_from_no_embed_metadata(&self) -> bool;
}

impl CrateTypeExt for CrateType {
    fn can_lto(&self) -> bool {
        match self {
            CrateType::Bin | CrateType::Staticlib | CrateType::Cdylib => true,
            CrateType::Lib
            | CrateType::Rlib
            | CrateType::Dylib
            | CrateType::ProcMacro
            | CrateType::Other(..) => false,
        }
    }

    fn is_linkable(&self) -> bool {
        match self {
            CrateType::Lib | CrateType::Rlib | CrateType::Dylib | CrateType::ProcMacro => true,
            CrateType::Bin | CrateType::Cdylib | CrateType::Staticlib | CrateType::Other(..) => {
                false
            }
        }
    }

    fn is_dynamic(&self) -> bool {
        match self {
            CrateType::Dylib | CrateType::Cdylib | CrateType::ProcMacro => true,
            CrateType::Lib
            | CrateType::Rlib
            | CrateType::Bin
            | CrateType::Staticlib
            | CrateType::Other(..) => false,
        }
    }

    fn requires_upstream_objects(&self) -> bool {
        // "lib" == "rlib" and is a compilation that doesn't actually
        // require upstream object files to exist, only upstream metadata
        // files. As a result, it doesn't require upstream artifacts

        !matches!(self, CrateType::Lib | CrateType::Rlib)
        // Everything else, however, is some form of "linkable output" or
        // something that requires upstream object files.
    }

    fn benefits_from_no_embed_metadata(&self) -> bool {
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
