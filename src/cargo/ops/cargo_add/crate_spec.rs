//! Crate name parsing.

use anyhow::Context as _;

use super::Dependency;
use crate::util::toml_mut::dependency::RegistrySource;
use crate::util::validate_package_name;
use crate::CargoResult;

/// User-specified crate
///
/// This can be a
/// - Name (e.g. `docopt`)
/// - Name and a version req (e.g. `docopt@^0.8`)
/// - Path
#[derive(Debug)]
pub struct CrateSpec {
    /// Crate name
    name: String,
    /// Optional version requirement
    version_req: Option<String>,
}

impl CrateSpec {
    /// Convert a string to a `Crate`
    pub fn resolve(pkg_id: &str) -> CargoResult<Self> {
        let (name, version) = pkg_id
            .split_once('@')
            .map(|(n, v)| (n, Some(v)))
            .unwrap_or((pkg_id, None));

        validate_package_name(name, "dependency name", "")?;

        if let Some(version) = version {
            semver::VersionReq::parse(version)
                .with_context(|| format!("invalid version requirement `{version}`"))?;
        }

        let id = Self {
            name: name.to_owned(),
            version_req: version.map(|s| s.to_owned()),
        };

        Ok(id)
    }

    /// Generate a dependency entry for this crate specifier
    pub fn to_dependency(&self) -> CargoResult<Dependency> {
        let mut dep = Dependency::new(self.name());
        if let Some(version_req) = self.version_req() {
            dep = dep.set_source(RegistrySource::new(version_req));
        }
        Ok(dep)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version_req(&self) -> Option<&str> {
        self.version_req.as_deref()
    }
}
