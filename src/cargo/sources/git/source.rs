use std::fmt;
use std::hash::sip::SipHasher;
use std::hash::Hasher;
use std::fmt::{Show,Formatter};
use std::io::MemWriter;
use serialize::hex::ToHex;
use url;
use url::Url;

use core::source::{Source,SourceId,GitKind};
use core::{Package,PackageId,Summary};
use util::{CargoResult,Config};
use sources::PathSource;
use sources::git::utils::{GitReference,GitRemote,Master,Other};

/* TODO: Refactor GitSource to delegate to a PathSource
 */
pub struct GitSource<'a, 'b> {
    remote: GitRemote,
    reference: GitReference,
    db_path: Path,
    checkout_path: Path,
    path_source: PathSource,
    config: &'a mut Config<'b>
}

impl<'a, 'b> GitSource<'a, 'b> {
    pub fn new<'a, 'b>(source_id: &SourceId, config: &'a mut Config<'b>) -> GitSource<'a, 'b> {
        assert!(source_id.is_git(), "id is not git, id={}", source_id);

        let reference = match source_id.kind {
            GitKind(ref reference) => reference,
            _ => fail!("Not a git source; id={}", source_id)
        };

        let remote = GitRemote::new(source_id.get_url());
        let ident = ident(&source_id.url);

        let db_path = config.git_db_path()
            .join(ident.as_slice());

        let checkout_path = config.git_checkout_path()
            .join(ident.as_slice()).join(reference.as_slice());

        let path_source = PathSource::new(&checkout_path, source_id);

        GitSource {
            remote: remote,
            reference: GitReference::for_str(reference.as_slice()),
            db_path: db_path,
            checkout_path: checkout_path,
            path_source: path_source,
            config: config
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

impl<'a, 'b> Show for GitSource<'a, 'b> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "git repo at {}", self.remote.get_url()));

        match self.reference {
            Master => Ok(()),
            Other(ref reference) => write!(f, " ({})", reference)
        }
    }
}

impl<'a, 'b> Source for GitSource<'a, 'b> {
    fn update(&mut self) -> CargoResult<()> {
        let should_update = self.config.update_remotes() || {
            !self.remote.has_ref(&self.db_path, self.reference.as_slice()).is_ok()
        };

        let repo = if should_update {
            try!(self.config.shell().status("Updating",
                format!("git repository `{}`", self.remote.get_url())));

            log!(5, "updating git source `{}`", self.remote);
            try!(self.remote.checkout(&self.db_path))
        } else {
            self.remote.db_at(&self.db_path)
        };

        try!(repo.copy_to(self.reference.as_slice(), &self.checkout_path));

        self.path_source.update()
    }

    fn list(&self) -> CargoResult<Vec<Summary>> {
        self.path_source.list()
    }

    fn download(&self, _: &[PackageId]) -> CargoResult<()> {
        // TODO: assert! that the PackageId is contained by the source
        Ok(())
    }

    fn get(&self, ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages for package ids `{}` from `{}`", ids, self.remote);
        self.path_source.get(ids)
    }

    fn fingerprint(&self) -> CargoResult<String> {
        let db = self.remote.db_at(&self.db_path);
        db.rev_for(self.reference.as_slice())
    }
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
