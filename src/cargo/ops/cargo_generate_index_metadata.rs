use crate::core::Workspace;
use crate::sources::registry::RegistryPackage;
use crate::util::errors::{CargoResult, CargoResultExt};

/// Generate index metadata for packages
pub fn generate_index_metadata<'a>(ws: &Workspace<'_>) -> CargoResult<RegistryPackage<'a>> {
    let pkg = ws.current()?;
    let config = ws.config();
    let filename = format!("{}-{}.crate", pkg.name(), pkg.version());
    let dir = ws.target_dir().join("package");

    dir.open_ro(filename, config, "package file")
        .and_then(|pkg_file| RegistryPackage::from_package(pkg, pkg_file))
        .chain_err(|| {
            "could not find package file. Ensure that crate has been packaged using `cargo package`"
        })
        .map_err(Into::into)
}
