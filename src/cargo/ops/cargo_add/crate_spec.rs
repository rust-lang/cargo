//! Crate name parsing.

use anyhow::Context as _;

use super::Dependency;
use crate::CargoResult;
use crate::util::toml_mut::dependency::RegistrySource;
use cargo_util_schemas::manifest::PackageName;

/// A user-provided version selector from `<name>@<value>`.
#[derive(Debug)]
pub(super) enum VersionSpec {
    /// A semver requirement that can be written to the manifest.
    Requirement(String),
    /// The special `@latest` selector, used for diagnostics only.
    Latest,
}

impl std::fmt::Display for VersionSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requirement(req) => req.fmt(f),
            Self::Latest => "latest".fmt(f),
        }
    }
}

/// User-specified crate
///
/// This can be a
/// - Name (e.g. `docopt`)
/// - Name and a version req (e.g. `docopt@^0.8`)
#[derive(Debug)]
pub struct CrateSpec {
    /// Crate name
    name: String,
    /// Optional version selector
    version: Option<VersionSpec>,
}

impl CrateSpec {
    /// Convert a string to a `Crate`
    pub fn resolve(pkg_id: &str) -> CargoResult<Self> {
        let (name, version) = pkg_id
            .split_once('@')
            .map(|(n, v)| (n, Some(v)))
            .unwrap_or((pkg_id, None));

        let package_name = PackageName::new(name);
        if !pkg_id.contains("@") && package_name.is_err() {
            for (idx, ch) in pkg_id.char_indices() {
                if !(unicode_ident::is_xid_continue(ch) || ch == '-') {
                    let mut suggested_pkg_id = pkg_id.to_string();
                    suggested_pkg_id.insert_str(idx, "@");
                    if let Ok(_) = CrateSpec::resolve(&suggested_pkg_id.as_str()) {
                        let err = package_name.unwrap_err();
                        return Err(
                            anyhow::format_err!("{err}\n\n\
                                help: if this is meant to be a package name followed by a version, insert an `@` like `{suggested_pkg_id}`").into());
                    }
                }
            }
        }

        package_name?;

        let version = if let Some(version) = version {
            // `latest` is the only supported special version selector. It is
            // not a SemVer requirement.
            //
            // We intentionally keep it case-sensitive to match other package
            // managers we may be helping users transition from.
            if version == "latest" {
                Some(VersionSpec::Latest)
            } else {
                semver::VersionReq::parse(version).with_context(|| {
                    if let Some(stripped) = version.strip_prefix("v") {
                        return format!(
                            "the version provided, `{version}` is not a \
                             valid SemVer requirement\n\n\
                             help: changing the package to `{name}@{stripped}`",
                        );
                    }
                    format!("invalid version requirement `{version}`")
                })?;
                Some(VersionSpec::Requirement(version.to_owned()))
            }
        } else {
            None
        };

        let id = Self {
            name: name.to_owned(),
            version,
        };

        Ok(id)
    }

    /// Generate a dependency entry for this crate specifier
    pub fn to_dependency(&self) -> CargoResult<Dependency> {
        let mut dep = Dependency::new(self.name());
        match self.version.as_ref() {
            Some(VersionSpec::Latest) => {
                anyhow::bail!("`latest` is not a valid dependency requirement")
            }
            Some(VersionSpec::Requirement(req)) => {
                dep = dep.set_source(RegistrySource::new(req));
            }
            None => {}
        }
        Ok(dep)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn version(&self) -> Option<&VersionSpec> {
        self.version.as_ref()
    }
}
