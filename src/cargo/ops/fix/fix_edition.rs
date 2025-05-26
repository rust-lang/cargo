//! Support for the permanently unstable `-Zfix-edition` flag.

use super::{EditionFixMode, FixOptions};
use crate::core::features::{Edition, FixEdition};
use crate::core::{Package, Workspace};
use crate::ops;
use crate::util::toml_mut::manifest::LocalManifest;
use crate::{CargoResult, GlobalContext};
use toml_edit::{Formatted, Item, Value};

/// Performs the actions for the `-Zfix-edition` flag.
pub fn fix_edition(
    gctx: &GlobalContext,
    original_ws: &Workspace<'_>,
    opts: &mut FixOptions,
    fix_edition: &FixEdition,
) -> CargoResult<()> {
    let packages = opts.compile_opts.spec.get_packages(original_ws)?;
    let skip_if_not_edition = |edition| -> CargoResult<bool> {
        if !packages.iter().all(|p| p.manifest().edition() == edition) {
            gctx.shell().status(
                "Skipping",
                &format!("not all packages are at edition {edition}"),
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    };

    match fix_edition {
        FixEdition::Start(edition) => {
            // The start point just runs `cargo check` if the edition is the
            // starting edition. This is so that crater can set a baseline of
            // whether or not the package builds at all. For other editions,
            // we skip entirely since they are not of interest since we can't
            // migrate them.
            if skip_if_not_edition(*edition)? {
                return Ok(());
            }
            ops::compile(&original_ws, &opts.compile_opts)?;
        }
        FixEdition::End { initial, next } => {
            // Skip packages that are not the starting edition, since we can
            // only migrate from one edition to the next.
            if skip_if_not_edition(*initial)? {
                return Ok(());
            }
            // Do the edition fix.
            opts.edition = Some(EditionFixMode::OverrideSpecific(*next));
            opts.allow_dirty = true;
            opts.allow_no_vcs = true;
            opts.allow_staged = true;
            ops::fix(gctx, original_ws, opts)?;
            // Do `cargo check` with the new edition so that we can verify
            // that it also works on the next edition.
            replace_edition(&packages, *next)?;
            gctx.shell()
                .status("Updated", &format!("edition to {next}"))?;
            let ws = original_ws.reload(gctx)?;
            // Unset these since we just want to do a normal `cargo check`.
            *opts
                .compile_opts
                .build_config
                .rustfix_diagnostic_server
                .borrow_mut() = None;
            opts.compile_opts.build_config.primary_unit_rustc = None;

            ops::compile(&ws, &opts.compile_opts)?;
        }
    }
    Ok(())
}

/// Modifies the `edition` value of the given packages to the new edition.
fn replace_edition(packages: &[&Package], to_edition: Edition) -> CargoResult<()> {
    for package in packages {
        let mut manifest_mut = LocalManifest::try_new(package.manifest_path())?;
        let document = &mut manifest_mut.data;
        let root = document.as_table_mut();
        // Update edition to the new value.
        if let Some(package) = root.get_mut("package").and_then(|t| t.as_table_like_mut()) {
            package.insert(
                "edition",
                Item::Value(Value::String(Formatted::new(to_edition.to_string()))),
            );
        }
        // If the edition is unstable, add it to cargo-features.
        if !to_edition.is_stable() {
            let feature = "unstable-editions";

            if let Some(features) = root
                .entry("cargo-features")
                .or_insert_with(|| Item::Value(Value::Array(toml_edit::Array::new())))
                .as_array_mut()
            {
                if !features
                    .iter()
                    .any(|f| f.as_str().map_or(false, |f| f == feature))
                {
                    features.push(feature);
                }
            }
        }
        manifest_mut.write()?;
    }
    Ok(())
}
