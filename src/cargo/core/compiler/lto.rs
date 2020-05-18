use crate::core::compiler::{Context, Unit};
use crate::core::interning::InternedString;
use crate::core::profiles;
use crate::core::TargetKind;
use crate::util::errors::CargoResult;
use std::collections::hash_map::{Entry, HashMap};

/// Possible ways to run rustc and request various parts of LTO.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum Lto {
    /// LTO is run for this rustc, and it's `-Clto=foo` where `foo` is optional.
    Run(Option<InternedString>),

    /// This rustc invocation only needs to produce bitcode, there's no need to
    /// produce object files, so we can pass `-Clinker-plugin-lto`
    OnlyBitcode,

    /// This rustc invocation needs to embed bitcode in object files. This means
    /// that object files may be used for a normal link, and the crate may be
    /// loaded for LTO later, so both are required.
    EmbedBitcode,

    /// Nothing related to LTO is required of this compilation.
    None,
}

pub fn generate(cx: &mut Context<'_, '_>) -> CargoResult<()> {
    let mut map = HashMap::new();
    for unit in cx.bcx.roots.iter() {
        calculate(cx, &mut map, unit, Lto::None)?;
    }
    cx.lto = map;
    Ok(())
}

fn calculate(
    cx: &Context<'_, '_>,
    map: &mut HashMap<Unit, Lto>,
    unit: &Unit,
    lto_for_deps: Lto,
) -> CargoResult<()> {
    let (lto, lto_for_deps) = if unit.target.for_host() {
        // Disable LTO for host builds since we only really want to perform LTO
        // for the final binary, and LTO on plugins/build scripts/proc macros is
        // largely not desired.
        (Lto::None, Lto::None)
    } else if unit.target.is_linkable() {
        // A "linkable" target is one that produces and rlib or dylib in this
        // case. In this scenario we cannot pass `-Clto` to the compiler because
        // that is an invalid request, this is simply a dependency. What we do,
        // however, is respect the request for whatever dependencies need to
        // have.
        //
        // Here if no LTO is requested then we keep it turned off. Otherwise LTO
        // is requested in some form, which means ideally we need just what's
        // requested, nothing else. It's possible, though, to have libraries
        // which are both a cdylib and and rlib, for example, which means that
        // object files are getting sent to the linker. That means that we need
        // to fully embed bitcode rather than simply generating just bitcode.
        let has_non_linkable_lib = match unit.target.kind() {
            TargetKind::Lib(kinds) => kinds.iter().any(|k| !k.is_linkable()),
            _ => true,
        };
        match lto_for_deps {
            Lto::None => (Lto::None, Lto::None),
            _ if has_non_linkable_lib => (Lto::EmbedBitcode, Lto::EmbedBitcode),
            other => (other, other),
        }
    } else {
        // Otherwise this target can perform LTO and we're going to read the
        // LTO value out of the profile. Note that we ignore `lto_for_deps`
        // here because if a unit depends on another unit than can LTO this
        // isn't a rustc-level dependency but rather a Cargo-level dependency.
        // For example this is an integration test depending on a binary.
        match unit.profile.lto {
            profiles::Lto::Named(s) => match s.as_str() {
                "n" | "no" | "off" => (Lto::Run(Some(s)), Lto::None),
                _ => (Lto::Run(Some(s)), Lto::OnlyBitcode),
            },
            profiles::Lto::Bool(true) => (Lto::Run(None), Lto::OnlyBitcode),
            profiles::Lto::Bool(false) => (Lto::None, Lto::None),
        }
    };

    match map.entry(unit.clone()) {
        // If we haven't seen this unit before then insert our value and keep
        // going.
        Entry::Vacant(v) => {
            v.insert(lto);
        }

        Entry::Occupied(mut v) => {
            let result = match (lto, v.get()) {
                // Once we're running LTO we keep running LTO. We should always
                // calculate the same thing here each iteration because if we
                // see this twice then it means, for example, two unit tests
                // depend on a binary, which is normal.
                (Lto::Run(s), _) | (_, &Lto::Run(s)) => Lto::Run(s),

                // If we calculated the same thing as before then we can bail
                // out quickly.
                (Lto::OnlyBitcode, Lto::OnlyBitcode) | (Lto::None, Lto::None) => return Ok(()),

                // This is where the trickiness happens. This unit needs
                // bitcode and the previously calculated value for this unit
                // says it didn't need bitcode (or vice versa). This means that
                // we're a shared dependency between some targets which require
                // LTO and some which don't. This means that instead of being
                // either only-objects or only-bitcode we have to embed both in
                // rlibs (used for different compilations), so we switch to
                // embedding bitcode.
                (Lto::OnlyBitcode, Lto::None) | (Lto::None, Lto::OnlyBitcode) => Lto::EmbedBitcode,

                // Once a target has requested bitcode embedding that's the
                // maximal amount of work that can be done, so we just keep
                // doing that work.
                (Lto::EmbedBitcode, _) | (_, Lto::EmbedBitcode) => Lto::EmbedBitcode,
            };
            // No need to recurse if we calculated the same value as before.
            if result == *v.get() {
                return Ok(());
            }
            v.insert(result);
        }
    }

    for dep in cx.unit_deps(unit) {
        calculate(cx, map, &dep.unit, lto_for_deps)?;
    }
    Ok(())
}
