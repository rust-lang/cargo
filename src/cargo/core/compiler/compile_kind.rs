use crate::core::compiler::BuildContext;
use crate::core::{InternedString, Target};
use crate::util::errors::{CargoResult, CargoResultExt};
use serde::Serialize;
use std::path::Path;

/// Indicator for how a unit is being compiled.
///
/// This is used primarily for organizing cross compilations vs host
/// compilations, where cross compilations happen at the request of `--target`
/// and host compilations happen for things like build scripts and procedural
/// macros.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord, Serialize)]
pub enum CompileKind {
    /// Attached to a unit that is compiled for the "host" system or otherwise
    /// is compiled without a `--target` flag. This is used for procedural
    /// macros and build scripts, or if the `--target` flag isn't passed.
    Host,

    /// Attached to a unit to be compiled for a particular target. This is used
    /// for units when the `--target` flag is passed.
    Target(CompileTarget),
}

impl CompileKind {
    pub fn is_host(&self) -> bool {
        match self {
            CompileKind::Host => true,
            _ => false,
        }
    }

    pub fn for_target(self, target: &Target) -> CompileKind {
        // Once we start compiling for the `Host` kind we continue doing so, but
        // if we are a `Target` kind and then we start compiling for a target
        // that needs to be on the host we lift ourselves up to `Host`.
        match self {
            CompileKind::Host => CompileKind::Host,
            CompileKind::Target(_) if target.for_host() => CompileKind::Host,
            CompileKind::Target(n) => CompileKind::Target(n),
        }
    }

    /// Returns a "short" name for this kind, suitable for keying off
    /// configuration in Cargo or presenting to users.
    pub fn short_name(&self, bcx: &BuildContext<'_, '_>) -> &str {
        match self {
            CompileKind::Host => bcx.host_triple().as_str(),
            CompileKind::Target(target) => target.short_name(),
        }
    }
}

/// Abstraction for the representation of a compilation target that Cargo has.
///
/// Compilation targets are one of two things right now:
///
/// 1. A raw target string, like `x86_64-unknown-linux-gnu`.
/// 2. The path to a JSON file, such as `/path/to/my-target.json`.
///
/// Raw target strings are typically dictated by `rustc` itself and represent
/// built-in targets. Custom JSON files are somewhat unstable, but supported
/// here in Cargo. Note that for JSON target files this `CompileTarget` stores a
/// full canonicalized path to the target.
///
/// The main reason for this existence is to handle JSON target files where when
/// we call rustc we pass full paths but when we use it for Cargo's purposes
/// like naming directories or looking up configuration keys we only check the
/// file stem of JSON target files. For built-in rustc targets this is just an
/// uninterpreted string basically.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord, Serialize)]
pub struct CompileTarget {
    name: InternedString,
}

impl CompileTarget {
    pub fn new(name: &str) -> CargoResult<CompileTarget> {
        let name = name.trim();
        if name.is_empty() {
            failure::bail!("target was empty");
        }
        if !name.ends_with(".json") {
            return Ok(CompileTarget { name: name.into() });
        }

        // If `name` ends in `.json` then it's likely a custom target
        // specification. Canonicalize the path to ensure that different builds
        // with different paths always produce the same result.
        let path = Path::new(name)
            .canonicalize()
            .chain_err(|| failure::format_err!("target path {:?} is not a valid file", name))?;

        let name = path
            .into_os_string()
            .into_string()
            .map_err(|_| failure::format_err!("target path is not valid unicode"))?;
        Ok(CompileTarget { name: name.into() })
    }

    /// Returns the full unqualified name of this target, suitable for passing
    /// to `rustc` directly.
    ///
    /// Typically this is pretty much the same as `short_name`, but for the case
    /// of JSON target files this will be a full canonicalized path name for the
    /// current filesystem.
    pub fn rustc_target(&self) -> &str {
        &self.name
    }

    /// Returns a "short" version of the target name suitable for usage within
    /// Cargo for configuration and such.
    ///
    /// This is typically the same as `rustc_target`, or the full name, but for
    /// JSON target files this returns just the file stem (e.g. `foo` out of
    /// `foo.json`) instead of the full path.
    pub fn short_name(&self) -> &str {
        // Flexible target specifications often point at json files, so if it
        // looks like we've got one of those just use the file stem (the file
        // name without ".json") as a short name for this target. Note that the
        // `unwrap()` here should never trigger since we have a nonempty name
        // and it starts as utf-8 so it's always utf-8
        if self.name.ends_with(".json") {
            Path::new(&self.name).file_stem().unwrap().to_str().unwrap()
        } else {
            &self.name
        }
    }
}
