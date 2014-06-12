use ops;
use url;
use core::source::Source;
use core::{Package,PackageId,Summary};
use util::CargoResult;
use sources::git::utils::{GitReference,GitRemote,Master,Other};
use std::fmt;
use std::fmt::{Show,Formatter};

pub struct GitSource {
    remote: GitRemote,
    reference: GitReference,
    db_path: Path,
    checkout_path: Path
}

impl GitSource {
    pub fn new(remote: GitRemote, reference: String, db: Path, checkout: Path) -> GitSource {
        GitSource { remote: remote, reference: GitReference::for_str(reference), db_path: db, checkout_path: checkout }
    }

    pub fn get_namespace<'a>(&'a self) -> &'a url::Url {
        self.remote.get_url()
    }
}

impl Show for GitSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "git repo at {}", self.remote.get_url()));

        match self.reference {
            Master => Ok(()),
            Other(ref reference) => write!(f, " ({})", reference)
        }
    }
}

impl Source for GitSource {
    fn update(&self) -> CargoResult<()> {
        log!(5, "updating git source `{}`", self.remote);
        let repo = try!(self.remote.checkout(&self.db_path));
        try!(repo.copy_to(self.reference.as_slice(), &self.checkout_path));

        Ok(())
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        log!(5, "listing summaries in git source `{}`", self.remote);
        let pkg = try!(read_manifest(&self.checkout_path, self.get_namespace()));
        Ok(vec!(pkg.get_summary().clone()))
    }

    fn download(&self, _: &[PackageId]) -> CargoResult<()> {
        Ok(())
    }

    fn get(&self, package_ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages for package ids `{}` from `{}`", package_ids, self.remote);
        // TODO: Support multiple manifests per repo
        let pkg = try!(read_manifest(&self.checkout_path, self.remote.get_url()));

        if package_ids.iter().any(|pkg_id| pkg_id == pkg.get_package_id()) {
            Ok(vec!(pkg))
        } else {
            Ok(vec!())
        }
    }
}

fn read_manifest(path: &Path, url: &url::Url) -> CargoResult<Package> {
    let path = path.join("Cargo.toml");
    ops::read_package(&path, url)
}
