use std::cell::{Ref, RefCell};
use std::collections::{HashMap, BTreeMap};
use std::fmt;
use std::hash;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::ser;
use toml;

use core::{Dependency, Manifest, PackageId, SourceId, Target};
use core::{Summary, SourceMap};
use ops;
use util::{Config, LazyCell, internal, lev_distance};
use util::errors::{CargoResult, CargoResultExt};

/// Information about a package that is available somewhere in the file system.
///
/// A package is a `Cargo.toml` file plus all the files that are part of it.
// TODO: Is manifest_path a relic?
#[derive(Clone, Debug)]
pub struct Package {
    /// The package's manifest
    manifest: Manifest,
    /// The root of the package
    manifest_path: PathBuf,
}

/// A Package in a form where `Serialize` can be derived.
#[derive(Serialize)]
struct SerializedPackage<'a> {
    name: &'a str,
    version: &'a str,
    id: &'a PackageId,
    license: Option<&'a str>,
    license_file: Option<&'a str>,
    description: Option<&'a str>,
    source: &'a SourceId,
    dependencies: &'a [Dependency],
    targets: &'a [Target],
    features: &'a BTreeMap<String, Vec<String>>,
    manifest_path: &'a str,
}

impl ser::Serialize for Package {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        let summary = self.manifest.summary();
        let package_id = summary.package_id();
        let manmeta = self.manifest.metadata();
        let license = manmeta.license.as_ref().map(String::as_ref);
        let license_file = manmeta.license_file.as_ref().map(String::as_ref);
        let description = manmeta.description.as_ref().map(String::as_ref);

        SerializedPackage {
            name: package_id.name(),
            version: &package_id.version().to_string(),
            id: package_id,
            license: license,
            license_file: license_file,
            description: description,
            source: summary.source_id(),
            dependencies: summary.dependencies(),
            targets: self.manifest.targets(),
            features: summary.features(),
            manifest_path: &self.manifest_path.display().to_string(),
        }.serialize(s)
    }
}

impl Package {
    /// Create a package from a manifest and its location
    pub fn new(manifest: Manifest,
               manifest_path: &Path) -> Package {
        Package {
            manifest: manifest,
            manifest_path: manifest_path.to_path_buf(),
        }
    }

    /// Calculate the Package from the manifest path (and cargo configuration).
    pub fn for_path(manifest_path: &Path, config: &Config) -> CargoResult<Package> {
        let path = manifest_path.parent().unwrap();
        let source_id = SourceId::for_path(path)?;
        let (pkg, _) = ops::read_package(manifest_path, &source_id, config)?;
        Ok(pkg)
    }

    /// Get the manifest dependencies
    pub fn dependencies(&self) -> &[Dependency] { self.manifest.dependencies() }
    /// Get the manifest
    pub fn manifest(&self) -> &Manifest { &self.manifest }
    /// Get the path to the manifest
    pub fn manifest_path(&self) -> &Path { &self.manifest_path }
    /// Get the name of the package
    pub fn name(&self) -> &str { self.package_id().name() }
    /// Get the PackageId object for the package (fully defines a packge)
    pub fn package_id(&self) -> &PackageId { self.manifest.package_id() }
    /// Get the root folder of the package
    pub fn root(&self) -> &Path { self.manifest_path.parent().unwrap() }
    /// Get the summary for the package
    pub fn summary(&self) -> &Summary { self.manifest.summary() }
    /// Get the targets specified in the manifest
    pub fn targets(&self) -> &[Target] { self.manifest.targets() }
    /// Get the current package version
    pub fn version(&self) -> &Version { self.package_id().version() }
    /// Get the package authors
    pub fn authors(&self) -> &Vec<String> { &self.manifest.metadata().authors }
    /// Whether the package is set to publish
    pub fn publish(&self) -> bool { self.manifest.publish() }

    /// Whether the package uses a custom build script for any target
    pub fn has_custom_build(&self) -> bool {
        self.targets().iter().any(|t| t.is_custom_build())
    }

    pub fn find_closest_target(&self,
                               target: &str,
                               is_expected_kind: fn(&Target)-> bool) -> Option<&Target> {
        let targets = self.targets();

        let matches = targets.iter().filter(|t| is_expected_kind(t))
                                    .map(|t| (lev_distance(target, t.name()), t))
                                    .filter(|&(d, _)| d < 4);
        matches.min_by_key(|t| t.0).map(|t| t.1)
    }

    pub fn map_source(self, to_replace: &SourceId, replace_with: &SourceId)
                      -> Package {
        Package {
            manifest: self.manifest.map_source(to_replace, replace_with),
            manifest_path: self.manifest_path,
        }
    }

    pub fn to_registry_toml(&self) -> String {
        let manifest = self.manifest().original().prepare_for_publish();
        let toml = toml::to_string(&manifest).unwrap();
        format!("\
            # THIS FILE IS AUTOMATICALLY GENERATED BY CARGO\n\
            #\n\
            # When uploading crates to the registry Cargo will automatically\n\
            # \"normalize\" Cargo.toml files for maximal compatibility\n\
            # with all versions of Cargo and also rewrite `path` dependencies\n\
            # to registry (e.g. crates.io) dependencies\n\
            #\n\
            # If you believe there's an error in this file please file an\n\
            # issue against the rust-lang/cargo repository. If you're\n\
            # editing this file be aware that the upstream Cargo.toml\n\
            # will likely look very different (and much more reasonable)\n\
            \n\
            {}\
        ", toml)
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.summary().package_id())
    }
}

impl PartialEq for Package {
    fn eq(&self, other: &Package) -> bool {
        self.package_id() == other.package_id()
    }
}

impl Eq for Package {}

impl hash::Hash for Package {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.package_id().hash(into)
    }
}

pub struct PackageSet<'cfg> {
    packages: HashMap<PackageId, LazyCell<Package>>,
    sources: RefCell<SourceMap<'cfg>>,
}

impl<'cfg> PackageSet<'cfg> {
    pub fn new(package_ids: &[PackageId],
               sources: SourceMap<'cfg>) -> PackageSet<'cfg> {
        PackageSet {
            packages: package_ids.iter().map(|id| {
                (id.clone(), LazyCell::new())
            }).collect(),
            sources: RefCell::new(sources),
        }
    }

    pub fn package_ids<'a>(&'a self) -> Box<Iterator<Item=&'a PackageId> + 'a> {
        Box::new(self.packages.keys())
    }

    pub fn get(&self, id: &PackageId) -> CargoResult<&Package> {
        let slot = self.packages.get(id).ok_or_else(|| {
            internal(format!("couldn't find `{}` in package set", id))
        })?;
        if let Some(pkg) = slot.borrow() {
            return Ok(pkg)
        }
        let mut sources = self.sources.borrow_mut();
        let source = sources.get_mut(id.source_id()).ok_or_else(|| {
            internal(format!("couldn't find source for `{}`", id))
        })?;
        let pkg = source.download(id).chain_err(|| {
            "unable to get packages from source"
        })?;
        assert!(slot.fill(pkg).is_ok());
        Ok(slot.borrow().unwrap())
    }

    pub fn sources(&self) -> Ref<SourceMap<'cfg>> {
        self.sources.borrow()
    }
}
