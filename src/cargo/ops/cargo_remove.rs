//! Core of cargo-remove command

use crate::core::Package;
use crate::util::toml_mut::manifest::DepTable;
use crate::util::toml_mut::manifest::LocalManifest;
use crate::CargoResult;
use crate::Config;

/// Remove a dependency from a Cargo.toml manifest file.
#[derive(Debug)]
pub struct RemoveOptions<'a> {
    /// Configuration information for Cargo operations
    pub config: &'a Config,
    /// Package to remove dependencies from
    pub spec: &'a Package,
    /// Dependencies to remove
    pub dependencies: Vec<String>,
    /// Which dependency section to remove these from
    pub section: DepTable,
    /// Whether or not to actually write the manifest
    pub dry_run: bool,
}

/// Remove dependencies from a manifest
pub fn remove(options: &RemoveOptions<'_>) -> CargoResult<()> {
    let dep_table = options
        .section
        .to_table()
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

    let manifest_path = options.spec.manifest_path().to_path_buf();
    let mut manifest = LocalManifest::try_new(&manifest_path)?;

    for dep in &options.dependencies {
        let section = if dep_table.len() >= 3 {
            format!("{} for target `{}`", &dep_table[2], &dep_table[1])
        } else {
            dep_table[0].clone()
        };
        options
            .config
            .shell()
            .status("Removing", format!("{dep} from {section}"))?;

        manifest.remove_from_table(&dep_table, dep)?;

        // Now that we have removed the crate, if that was the last reference to that
        // crate, then we need to drop any explicitly activated features on
        // that crate.
        manifest.gc_dep(dep);
    }

    if options.dry_run {
        options
            .config
            .shell()
            .warn("aborting remove due to dry run")?;
    } else {
        manifest.write()?;
    }

    Ok(())
}
