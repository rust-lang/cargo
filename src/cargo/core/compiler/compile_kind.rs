use crate::core::compiler::BuildContext;
use crate::core::{InternedString, Target};
use crate::util::errors::{CargoResult, CargoResultExt};
use serde::Serialize;
use std::path::Path;

/// Indicates whether an object is for the host architcture or the target architecture.
///
/// These will be the same unless cross-compiling.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord, Serialize)]
pub enum CompileKind {
    Host,
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

    pub fn rustc_target(&self) -> &str {
        &self.name
    }

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
