use std::fmt;
use std::hash::sip::SipHasher;
use std::hash::Hasher;
use std::fmt::{Show,Formatter};
use std::io::MemWriter;
use serialize::hex::ToHex;
use url;
use url::Url;

use ops;
use core::source::Source;
use core::{Package,PackageId,Summary};
use util::{CargoResult,Config};
use sources::git::utils::{GitReference,GitRemote,Master,Other};

pub struct GitSource {
    remote: GitRemote,
    reference: GitReference,
    db_path: Path,
    checkout_path: Path
}

impl GitSource {
    pub fn new(url: &Url, reference: &str, config: &Config) -> GitSource {
        let remote = GitRemote::new(url);
        let ident = ident(url);

        let db_path = config.git_db_path()
            .join(ident.as_slice());

        let checkout_path = config.git_checkout_path()
            .join(ident.as_slice()).join(reference);

        GitSource {
            remote: remote,
            reference: GitReference::for_str(reference),
            db_path: db_path,
            checkout_path: checkout_path
        }
    }

    pub fn get_namespace<'a>(&'a self) -> &'a url::Url {
        self.remote.get_url()
    }

}

fn ident(url: &Url) -> String {
    let hasher = SipHasher::new_with_keys(0,0);

    let mut ident = url.path.as_slice().split('/').last().unwrap();

    ident = if ident == "" {
        "_empty"
    } else {
        ident
    };

    format!("{}-{}", ident, to_hex(hasher.hash(&url.to_str())))
}

fn to_hex(num: u64) -> String {
    let mut writer = MemWriter::with_capacity(8);
    writer.write_le_u64(num).unwrap(); // this should never fail
    writer.get_ref().to_hex()
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
        println!("Updating git repository `{}`", self.remote.get_url());
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

#[cfg(test)]
mod test {
    use url;
    use url::Url;
    use super::ident;

    #[test]
    pub fn test_url_to_path_ident_with_path() {
        let ident = ident(&url("https://github.com/carlhuda/cargo"));
        assert_eq!(ident.as_slice(), "cargo-0eed735c8ffd7c88");
    }

    #[test]
    pub fn test_url_to_path_ident_without_path() {
        let ident = ident(&url("https://github.com"));
        assert_eq!(ident.as_slice(), "_empty-fc065c9b6b16fc00");
    }


    fn url(s: &str) -> Url {
        url::from_str(s).unwrap()
    }
}
