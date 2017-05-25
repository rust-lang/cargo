use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use rustc_serialize::hex::ToHex;
use serde_json;

use core::{Package, PackageId, Summary, SourceId, Source, Dependency, Registry};
use sources::PathSource;
use util::{Config, Sha256};
use util::errors::{CargoResult, CargoResultExt};
use util::paths;

pub struct DirectorySource<'cfg> {
    source_id: SourceId,
    root: PathBuf,
    packages: HashMap<PackageId, (Package, Checksum)>,
    config: &'cfg Config,
}

#[derive(Deserialize)]
struct Checksum {
    package: String,
    files: HashMap<String, String>,
}

impl<'cfg> DirectorySource<'cfg> {
    pub fn new(path: &Path, id: &SourceId, config: &'cfg Config)
               -> DirectorySource<'cfg> {
        DirectorySource {
            source_id: id.clone(),
            root: path.to_path_buf(),
            config: config,
            packages: HashMap::new(),
        }
    }
}

impl<'cfg> Debug for DirectorySource<'cfg> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "DirectorySource {{ root: {:?} }}", self.root)
    }
}

impl<'cfg> Registry for DirectorySource<'cfg> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let packages = self.packages.values().map(|p| &p.0);
        let matches = packages.filter(|pkg| dep.matches(pkg.summary()));
        let summaries = matches.map(|pkg| pkg.summary().clone());
        Ok(summaries.collect())
    }

    fn supports_checksums(&self) -> bool {
        true
    }
}

impl<'cfg> Source for DirectorySource<'cfg> {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn update(&mut self) -> CargoResult<()> {
        self.packages.clear();
        let entries = self.root.read_dir().chain_err(|| {
            format!("failed to read root of directory source: {}",
                    self.root.display())
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Ignore hidden/dot directories as they typically don't contain
            // crates and otherwise may conflict with a VCS
            // (rust-lang/cargo#3414).
            if let Some(s) = path.file_name().and_then(|s| s.to_str()) {
                if s.starts_with('.') {
                    continue
                }
            }

            // Vendor directories are often checked into a VCS, but throughout
            // the lifetime of a vendor dir crates are often added and deleted.
            // Some VCS implementations don't always fully delete the directory
            // when a dir is removed from a different checkout. Sometimes a
            // mostly-empty dir is left behind.
            //
            // To help work Cargo work by default in more cases we try to
            // handle this case by default. If the directory looks like it only
            // has dotfiles in it (or no files at all) then we skip it.
            //
            // In general we don't want to skip completely malformed directories
            // to help with debugging, so we don't just ignore errors in
            // `update` below.
            let mut only_dotfile = true;
            for entry in path.read_dir()?.filter_map(|e| e.ok()) {
                if let Some(s) = entry.file_name().to_str() {
                    if s.starts_with(".") {
                        continue
                    }
                }
                only_dotfile = false;
            }
            if only_dotfile {
                continue
            }

            let mut src = PathSource::new(&path, &self.source_id, self.config);
            src.update()?;
            let pkg = src.root_package()?;

            let cksum_file = path.join(".cargo-checksum.json");
            let cksum = paths::read(&path.join(cksum_file)).chain_err(|| {
                format!("failed to load checksum `.cargo-checksum.json` \
                         of {} v{}",
                        pkg.package_id().name(),
                        pkg.package_id().version())

            })?;
            let cksum: Checksum = serde_json::from_str(&cksum)
                .chain_err(|| {
                format!("failed to decode `.cargo-checksum.json` of \
                         {} v{}",
                        pkg.package_id().name(),
                        pkg.package_id().version())
            })?;

            let mut manifest = pkg.manifest().clone();
            let summary = manifest.summary().clone();
            manifest.set_summary(summary.set_checksum(cksum.package.clone()));
            let pkg = Package::new(manifest, pkg.manifest_path());
            self.packages.insert(pkg.package_id().clone(), (pkg, cksum));
        }

        Ok(())
    }

    fn download(&mut self, id: &PackageId) -> CargoResult<Package> {
        self.packages.get(id).map(|p| &p.0).cloned().ok_or_else(|| {
            format!("failed to find package with id: {}", id).into()
        })
    }

    fn fingerprint(&self, pkg: &Package) -> CargoResult<String> {
        Ok(pkg.package_id().version().to_string())
    }

    fn verify(&self, id: &PackageId) -> CargoResult<()> {
        let (pkg, cksum) = match self.packages.get(id) {
            Some(&(ref pkg, ref cksum)) => (pkg, cksum),
            None => bail!("failed to find entry for `{}` in directory source",
                          id),
        };

        let mut buf = [0; 16 * 1024];
        for (file, cksum) in cksum.files.iter() {
            let mut h = Sha256::new();
            let file = pkg.root().join(file);

            (|| -> CargoResult<()> {
                let mut f = File::open(&file)?;
                loop {
                    match f.read(&mut buf)? {
                        0 => return Ok(()),
                        n => h.update(&buf[..n]),
                    }
                }
            })().chain_err(|| {
                format!("failed to calculate checksum of: {}",
                        file.display())
            })?;

            let actual = h.finish().to_hex();
            if &*actual != cksum {
                bail!("\
                    the listed checksum of `{}` has changed:\n\
                    expected: {}\n\
                    actual:   {}\n\
                    \n\
                    directory sources are not intended to be edited, if \
                    modifications are required then it is recommended \
                    that [replace] is used with a forked copy of the \
                    source\
                ", file.display(), cksum, actual);
            }
        }

        Ok(())
    }
}
