use crate::core::compiler::{BuildContext, CompileMode, CrateType, Unit};
use crate::core::profiles;
use crate::util::interning::InternedString;

use crate::util::errors::CargoResult;
use std::collections::hash_map::{Entry, HashMap};

/// Possible ways to run rustc and request various parts of [LTO].
///
/// Variant            | Flag                   | Object Code | Bitcode
/// -------------------|------------------------|-------------|--------
/// `Run`              | `-C lto=foo`           | n/a         | n/a
/// `Off`              | `-C lto=off`           | n/a         | n/a
/// `OnlyBitcode`      | `-C linker-plugin-lto` |             | ✓
/// `ObjectAndBitcode` |                        | ✓           | ✓
/// `OnlyObject`       | `-C embed-bitcode=no`  | ✓           |
///
/// [LTO]: https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#lto
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Lto {
    /// LTO is run for this rustc, and it's `-Clto=foo`. If the given value is
    /// None, that corresponds to `-Clto` with no argument, which means do
    /// "fat" LTO.
    Run(Option<InternedString>),

    /// LTO has been explicitly listed as "off". This means no thin-local-LTO,
    /// no LTO anywhere, I really mean it!
    Off,

    /// This rustc invocation only needs to produce bitcode (it is *only* used
    /// for LTO), there's no need to produce object files, so we can pass
    /// `-Clinker-plugin-lto`
    OnlyBitcode,

    /// This rustc invocation needs to embed bitcode in object files. This means
    /// that object files may be used for a normal link, and the crate may be
    /// loaded for LTO later, so both are required.
    ObjectAndBitcode,

    /// This should not include bitcode. This is primarily to reduce disk
    /// space usage.
    OnlyObject,
}

pub fn generate(bcx: &BuildContext<'_, '_>) -> CargoResult<HashMap<Unit, Lto>> {
    let mut map = HashMap::new();
    for unit in bcx.roots.iter() {
        let root_lto = match unit.profile.lto {
            // LTO not requested, no need for bitcode.
            profiles::Lto::Bool(false) => Lto::OnlyObject,
            profiles::Lto::Off => Lto::Off,
            _ => {
                let crate_types = unit.target.rustc_crate_types();
                if unit.target.for_host() {
                    Lto::OnlyObject
                } else if needs_object(&crate_types) {
                    lto_when_needs_object(&crate_types)
                } else {
                    // This may or may not participate in LTO, let's start
                    // with the minimum requirements. This may be expanded in
                    // `calculate` below if necessary.
                    Lto::OnlyBitcode
                }
            }
        };
        calculate(bcx, &mut map, unit, root_lto)?;
    }
    Ok(map)
}

/// Whether or not any of these crate types need object code.
fn needs_object(crate_types: &[CrateType]) -> bool {
    crate_types.iter().any(|k| k.can_lto() || k.is_dynamic())
}

/// Lto setting to use when this unit needs object code.
fn lto_when_needs_object(crate_types: &[CrateType]) -> Lto {
    if crate_types.iter().all(|ct| *ct == CrateType::Dylib) {
        // A dylib whose parent is running LTO. rustc currently
        // doesn't support LTO with dylibs, so bitcode is not
        // needed.
        Lto::OnlyObject
    } else {
        // Mixed rlib with a dylib or cdylib whose parent is running LTO. This
        // needs both: bitcode for the rlib (for LTO) and object code for the
        // dylib.
        Lto::ObjectAndBitcode
    }
}

fn calculate(
    bcx: &BuildContext<'_, '_>,
    map: &mut HashMap<Unit, Lto>,
    unit: &Unit,
    parent_lto: Lto,
) -> CargoResult<()> {
    let crate_types = match unit.mode {
        // Note: Doctest ignores LTO, but for now we'll compute it as-if it is
        // a Bin, in case it is ever supported in the future.
        CompileMode::Test | CompileMode::Bench | CompileMode::Doctest => vec![CrateType::Bin],
        // Notes on other modes:
        // - Check: Treat as the underlying type, it doesn't really matter.
        // - Doc: LTO is N/A for the Doc unit itself since rustdoc does not
        //   support codegen flags. We still compute the dependencies, which
        //   are mostly `Check`.
        // - RunCustomBuild is ignored because it is always "for_host".
        _ => unit.target.rustc_crate_types(),
    };
    // LTO can only be performed if *all* of the crate types support it.
    // For example, a cdylib/rlib combination won't allow LTO.
    let all_lto_types = crate_types.iter().all(CrateType::can_lto);
    // Compute the LTO based on the profile, and what our parent requires.
    let lto = if unit.target.for_host() {
        // Disable LTO for host builds since we only really want to perform LTO
        // for the final binary, and LTO on plugins/build scripts/proc macros is
        // largely not desired.
        Lto::OnlyObject
    } else if all_lto_types {
        // Note that this ignores the `parent_lto` because this isn't a
        // linkable crate type; this unit is not being embedded in the parent.
        match unit.profile.lto {
            profiles::Lto::Named(s) => Lto::Run(Some(s)),
            profiles::Lto::Off => Lto::Off,
            profiles::Lto::Bool(true) => Lto::Run(None),
            profiles::Lto::Bool(false) => Lto::OnlyObject,
        }
    } else {
        match (parent_lto, needs_object(&crate_types)) {
            // An rlib whose parent is running LTO, we only need bitcode.
            (Lto::Run(_), false) => Lto::OnlyBitcode,
            // LTO when something needs object code.
            (Lto::Run(_), true) | (Lto::OnlyBitcode, true) => lto_when_needs_object(&crate_types),
            // LTO is disabled, continue to disable it.
            (Lto::Off, _) => Lto::Off,
            // If this doesn't have any requirements, or the requirements are
            // already satisfied, then stay with our parent.
            (_, false) | (Lto::OnlyObject, true) | (Lto::ObjectAndBitcode, true) => parent_lto,
        }
    };

    // Merge the computed LTO. If this unit appears multiple times in the
    // graph, the merge may expand the requirements.
    let merged_lto = match map.entry(unit.clone()) {
        // If we haven't seen this unit before then insert our value and keep
        // going.
        Entry::Vacant(v) => *v.insert(lto),

        Entry::Occupied(mut v) => {
            let result = match (lto, v.get()) {
                // No change in requirements.
                (Lto::OnlyBitcode, Lto::OnlyBitcode) => Lto::OnlyBitcode,
                (Lto::OnlyObject, Lto::OnlyObject) => Lto::OnlyObject,

                // Once we're running LTO we keep running LTO. We should always
                // calculate the same thing here each iteration because if we
                // see this twice then it means, for example, two unit tests
                // depend on a binary, which is normal.
                (Lto::Run(s), _) | (_, &Lto::Run(s)) => Lto::Run(s),

                // Off means off! This has the same reasoning as `Lto::Run`.
                (Lto::Off, _) | (_, Lto::Off) => Lto::Off,

                // Once a target has requested both, that's the maximal amount
                // of work that can be done, so we just keep doing that work.
                (Lto::ObjectAndBitcode, _) | (_, Lto::ObjectAndBitcode) => Lto::ObjectAndBitcode,

                // Upgrade so that both requirements can be met.
                //
                // This is where the trickiness happens. This unit needs
                // bitcode and the previously calculated value for this unit
                // says it didn't need bitcode (or vice versa). This means that
                // we're a shared dependency between some targets which require
                // LTO and some which don't. This means that instead of being
                // either only-objects or only-bitcode we have to embed both in
                // rlibs (used for different compilations), so we switch to
                // including both.
                (Lto::OnlyObject, Lto::OnlyBitcode) | (Lto::OnlyBitcode, Lto::OnlyObject) => {
                    Lto::ObjectAndBitcode
                }
            };
            // No need to recurse if we calculated the same value as before.
            if result == *v.get() {
                return Ok(());
            }
            v.insert(result);
            result
        }
    };

    for dep in &bcx.unit_graph[unit] {
        calculate(bcx, map, &dep.unit, merged_lto)?;
    }
    Ok(())
}
