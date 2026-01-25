//! Type definitions for cross-compilation.

use crate::core::Target;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::{GlobalContext, StableHasher, try_canonicalize};
use anyhow::Context as _;
use anyhow::bail;
use cargo_util::ProcessBuilder;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Indicator for how a unit is being compiled.
///
/// This is used primarily for organizing cross compilations vs host
/// compilations, where cross compilations happen at the request of `--target`
/// and host compilations happen for things like build scripts and procedural
/// macros.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum CompileKind {
    /// Attached to a unit that is compiled for the "host" system or otherwise
    /// is compiled without a `--target` flag. This is used for procedural
    /// macros and build scripts, or if the `--target` flag isn't passed.
    Host,

    /// Attached to a unit to be compiled for a particular target. This is used
    /// for units when the `--target` flag is passed.
    Target(CompileTarget),
}

/// Fallback behavior in the
/// [`CompileKind::from_requested_targets_with_fallback`] function when
/// no targets are specified.
pub enum CompileKindFallback {
    /// The build configuration is consulted to find the default target, such as
    /// `$CARGO_BUILD_TARGET` or reading `build.target`.
    BuildConfig,

    /// Only the host should be returned when targets aren't explicitly
    /// specified. This is used by `cargo metadata` for example where "only
    /// host" has a special meaning in terms of the returned metadata.
    JustHost,
}

impl CompileKind {
    pub fn is_host(&self) -> bool {
        matches!(self, CompileKind::Host)
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

    /// Creates a new list of `CompileKind` based on the requested list of
    /// targets.
    ///
    /// If no targets are given then this returns a single-element vector with
    /// `CompileKind::Host`.
    pub fn from_requested_targets(
        gctx: &GlobalContext,
        targets: &[String],
    ) -> CargoResult<Vec<CompileKind>> {
        CompileKind::from_requested_targets_with_fallback(
            gctx,
            targets,
            CompileKindFallback::BuildConfig,
        )
    }

    /// Same as [`CompileKind::from_requested_targets`] except that if `targets`
    /// doesn't explicitly mention anything the behavior of what to return is
    /// controlled by the `fallback` argument.
    pub fn from_requested_targets_with_fallback(
        gctx: &GlobalContext,
        targets: &[String],
        fallback: CompileKindFallback,
    ) -> CargoResult<Vec<CompileKind>> {
        let dedup = |targets: &[String]| {
            let deduplicated_targets = targets
                .iter()
                .map(|value| {
                    // This neatly substitutes the manually-specified `host-tuple` target directive
                    // with the compiling machine's target triple.

                    if value.as_str() == "host-tuple" {
                        let host_triple = env!("RUST_HOST_TARGET");
                        Ok(CompileKind::Target(CompileTarget::new(host_triple)?))
                    } else {
                        Ok(CompileKind::Target(CompileTarget::new(value.as_str())?))
                    }
                })
                // First collect into a set to deduplicate any `--target` passed
                // more than once...
                .collect::<CargoResult<BTreeSet<_>>>()?
                // ... then generate a flat list for everything else to use.
                .into_iter()
                .collect();

            Ok(deduplicated_targets)
        };

        if !targets.is_empty() {
            return dedup(targets);
        }

        let kinds = match (fallback, &gctx.build_config()?.target) {
            (_, None) | (CompileKindFallback::JustHost, _) => Ok(vec![CompileKind::Host]),
            (CompileKindFallback::BuildConfig, Some(build_target_config)) => {
                dedup(&build_target_config.values(gctx.cwd())?)
            }
        };

        kinds
    }

    /// Hash used for fingerprinting.
    ///
    /// Metadata hashing uses the normal Hash trait, which does not
    /// differentiate on `.json` file contents. The fingerprint hash does
    /// check the contents.
    pub fn fingerprint_hash(&self) -> u64 {
        match self {
            CompileKind::Host => 0,
            CompileKind::Target(target) => target.fingerprint_hash(),
        }
    }

    /// Adds the `--target` flag to the given [`ProcessBuilder`] if this is a
    /// non-host build.
    pub fn add_target_arg(&self, builder: &mut ProcessBuilder) {
        if let CompileKind::Target(target) = self {
            builder.arg("--target").arg(target.rustc_target());
            if matches!(target, CompileTarget::Json { .. }) {
                builder.arg("-Zunstable-options");
            }
        }
    }
}

impl serde::ser::Serialize for CompileKind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            CompileKind::Host => None::<&str>.serialize(s),
            CompileKind::Target(t) => Some(t.rustc_target()).serialize(s),
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
pub enum CompileTarget {
    Tuple(InternedString),
    Json {
        short: InternedString,
        path: InternedString,
    },
}

impl CompileTarget {
    pub fn new(name: &str) -> CargoResult<CompileTarget> {
        let name = name.trim();
        if name.is_empty() {
            bail!("target was empty");
        }
        if !name.ends_with(".json") {
            return Ok(CompileTarget::Tuple(name.into()));
        }

        // If `name` ends in `.json` then it's likely a custom target
        // specification. Canonicalize the path to ensure that different builds
        // with different paths always produce the same result.
        let p = try_canonicalize(Path::new(name))
            .with_context(|| format!("target path `{name}` is not a valid file"))?;
        let path = p
            .to_str()
            .ok_or_else(|| anyhow::format_err!("target path `{name}` is not valid unicode"))?
            .into();
        let short = p.file_stem().unwrap().to_str().unwrap().into();
        Ok(CompileTarget::Json { short, path })
    }

    /// Returns the full unqualified name of this target, suitable for passing
    /// to `rustc` directly.
    ///
    /// Typically this is pretty much the same as `short_name`, but for the case
    /// of JSON target files this will be a full canonicalized path name for the
    /// current filesystem.
    pub fn rustc_target(&self) -> InternedString {
        match self {
            CompileTarget::Tuple(name) => *name,
            CompileTarget::Json { path, .. } => *path,
        }
    }

    /// Returns a "short" version of the target name suitable for usage within
    /// Cargo for configuration and such.
    ///
    /// This is typically the same as `rustc_target`, or the full name, but for
    /// JSON target files this returns just the file stem (e.g. `foo` out of
    /// `foo.json`) instead of the full path.
    pub fn short_name(&self) -> &str {
        match self {
            CompileTarget::Tuple(name) => name,
            CompileTarget::Json { short, .. } => short,
        }
    }

    /// See [`CompileKind::fingerprint_hash`].
    pub fn fingerprint_hash(&self) -> u64 {
        let mut hasher = StableHasher::new();
        match self {
            CompileTarget::Tuple(name) => name.hash(&mut hasher),
            CompileTarget::Json { path, .. } => {
                // This may have some performance concerns, since it is called
                // fairly often. If that ever seems worth fixing, consider
                // embedding this in `CompileTarget`.
                match fs::read_to_string(path) {
                    Ok(contents) => contents.hash(&mut hasher),
                    Err(_) => path.hash(&mut hasher),
                }
            }
        }
        Hasher::finish(&hasher)
    }
}
