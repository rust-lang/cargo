use crate::core::compiler::{Context, Unit};
use crate::core::interning::InternedString;
use crate::core::profiles;
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
        calculate(cx, &mut map, unit, false)?;
    }
    cx.lto = map;
    Ok(())
}

fn calculate(
    cx: &Context<'_, '_>,
    map: &mut HashMap<Unit, Lto>,
    unit: &Unit,
    require_bitcode: bool,
) -> CargoResult<()> {
    let (lto, require_bitcode_for_deps) = if unit.target.for_host() {
        // Disable LTO for host builds since we only really want to perform LTO
        // for the final binary, and LTO on plugins/build scripts/proc macros is
        // largely not desired.
        (Lto::None, false)
    } else if unit.target.can_lto() {
        // Otherwise if this target can perform LTO then we're going to read the
        // LTO value out of the profile.
        assert!(!require_bitcode); // can't depend on binaries/staticlib/etc
        match unit.profile.lto {
            profiles::Lto::Named(s) => match s.as_str() {
                "n" | "no" | "off" => (Lto::Run(Some(s)), false),
                _ => (Lto::Run(Some(s)), true),
            },
            profiles::Lto::Bool(true) => (Lto::Run(None), true),
            profiles::Lto::Bool(false) => (Lto::None, false),
        }
    } else if require_bitcode {
        // Otherwise we're a dependency of something, an rlib. This means that
        // if our parent required bitcode of some kind then we need to generate
        // bitcode.
        (Lto::OnlyBitcode, true)
    } else {
        (Lto::None, false)
    };

    match map.entry(unit.clone()) {
        // If we haven't seen this unit before then insert our value and keep
        // going.
        Entry::Vacant(v) => {
            v.insert(lto);
        }

        Entry::Occupied(mut v) => {
            let result = match (lto, v.get()) {
                // Targets which execute LTO cannot be depended on, so these
                // units should only show up once in the dependency graph, so we
                // should never hit this case.
                (Lto::Run(_), _) | (_, Lto::Run(_)) => {
                    unreachable!("lto-able targets shouldn't show up twice")
                }

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
                (Lto::OnlyBitcode, Lto::None)
                | (Lto::OnlyBitcode, Lto::EmbedBitcode)
                | (Lto::None, Lto::OnlyBitcode)
                | (Lto::None, Lto::EmbedBitcode) => Lto::EmbedBitcode,

                // Currently this variant is never calculated above, so no need
                // to handle this case.
                (Lto::EmbedBitcode, _) => unreachable!(),
            };
            // No need to recurse if we calculated the same value as before.
            if result == *v.get() {
                return Ok(());
            }
            v.insert(result);
        }
    }

    for dep in cx.unit_deps(unit) {
        calculate(cx, map, &dep.unit, require_bitcode_for_deps)?;
    }
    Ok(())
}
