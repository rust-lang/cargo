use std::fmt::{mod, Show, Formatter};
use std::hash::Hasher;
use std::hash::sip::SipHasher;
use std::mem;
use url::{mod, Url};

use core::source::{Source, SourceId, GitKind};
use core::{Package, PackageId, Summary, Registry, Dependency};
use util::{CargoResult, Config, to_hex};
use sources::PathSource;
use sources::git::utils::{GitReference, GitRemote, Master, Other, GitRevision};

/* TODO: Refactor GitSource to delegate to a PathSource
 */
pub struct GitSource<'a, 'b> {
    remote: GitRemote,
    reference: GitReference,
    db_path: Path,
    checkout_path: Path,
    source_id: SourceId,
    path_source: Option<PathSource>,
    rev: Option<GitRevision>,
    config: &'a mut Config<'b>,
}

impl<'a, 'b> GitSource<'a, 'b> {
    pub fn new<'a, 'b>(source_id: &SourceId,
                       config: &'a mut Config<'b>) -> GitSource<'a, 'b> {
        assert!(source_id.is_git(), "id is not git, id={}", source_id);

        let reference = match source_id.kind {
            GitKind(ref reference) => reference,
            _ => fail!("Not a git source; id={}", source_id)
        };

        let remote = GitRemote::new(source_id.get_url());
        let ident = ident(source_id.get_url());

        let db_path = config.git_db_path()
            .join(ident.as_slice());

        let checkout_path = config.git_checkout_path()
            .join(ident.as_slice()).join(reference.as_slice());

        let reference = match source_id.precise {
            Some(ref s) => s,
            None => reference,
        };

        GitSource {
            remote: remote,
            reference: GitReference::for_str(reference.as_slice()),
            db_path: db_path,
            checkout_path: checkout_path,
            source_id: source_id.clone(),
            path_source: None,
            rev: None,
            config: config,
        }
    }

    pub fn get_url(&self) -> &Url {
        self.remote.get_url()
    }
}

fn ident(url: &Url) -> String {
    let hasher = SipHasher::new_with_keys(0,0);

    // FIXME: this really should be able to not use to_str() everywhere, but the
    //        compiler seems to currently ask for static lifetimes spuriously.
    //        Perhaps related to rust-lang/rust#15144
    let url = canonicalize_url(url);
    let ident = url.path().unwrap_or(&[])
                   .last().map(|a| a.clone()).unwrap_or(String::new());

    let ident = if ident.as_slice() == "" {
        "_empty".to_string()
    } else {
        ident
    };

    format!("{}-{}", ident, to_hex(hasher.hash(&url)))
}

// Some hacks and heuristics for making equivalent URLs hash the same
pub fn canonicalize_url(url: &Url) -> Url {
    let mut url = url.clone();

    // Strip a trailing slash
    match url.scheme_data {
        url::RelativeSchemeData(ref mut rel) => {
            if rel.path.last().map(|s| s.is_empty()).unwrap_or(false) {
                rel.path.pop();
            }
        }
        _ => {}
    }

    // HACKHACK: For github URL's specifically just lowercase
    // everything.  GitHub treats both the same, but they hash
    // differently, and we're gonna be hashing them. This wants a more
    // general solution, and also we're almost certainly not using the
    // same case conversion rules that GitHub does. (#84)
    if url.domain() == Some("github.com") {
        url.scheme = "https".to_string();
        match url.scheme_data {
            url::RelativeSchemeData(ref mut rel) => {
                rel.port = Some(443);
                rel.default_port = Some(443);
                let path = mem::replace(&mut rel.path, Vec::new());
                rel.path = path.move_iter().map(|s| {
                    s.as_slice().chars().map(|c| c.to_lowercase()).collect()
                }).collect();
            }
            _ => {}
        }
    }

    // Repos generally can be accessed with or w/o '.git'
    match url.scheme_data {
        url::RelativeSchemeData(ref mut rel) => {
            let needs_chopping = {
                let last = rel.path.last().map(|s| s.as_slice()).unwrap_or("");
                last.ends_with(".git")
            };
            if needs_chopping {
                let last = rel.path.pop().unwrap();
                let last = last.as_slice();
                rel.path.push(last.slice_to(last.len() - 4).to_string())
            }
        }
        _ => {}
    }

    return url;
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

impl<'a, 'b> Registry for GitSource<'a, 'b> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let src = self.path_source.as_mut()
                      .expect("BUG: update() must be called before query()");
        src.query(dep)
    }
}

impl<'a, 'b> Source for GitSource<'a, 'b> {
    fn update(&mut self) -> CargoResult<()> {
        let actual_rev = self.remote.rev_for(&self.db_path,
                                             self.reference.as_slice());
        let should_update = actual_rev.is_err() ||
                            self.source_id.precise.is_none();

        let (repo, actual_rev) = if should_update {
            try!(self.config.shell().status("Updating",
                format!("git repository `{}`", self.remote.get_url())));

            log!(5, "updating git source `{}`", self.remote);
            let repo = try!(self.remote.checkout(&self.db_path));
            let rev = try!(repo.rev_for(self.reference.as_slice()));
            (repo, rev)
        } else {
            (try!(self.remote.db_at(&self.db_path)), actual_rev.unwrap())
        };

        try!(repo.copy_to(actual_rev.clone(), &self.checkout_path));

        let source_id = self.source_id.with_precise(actual_rev.to_string());
        let path_source = PathSource::new(&self.checkout_path, &source_id);

        self.path_source = Some(path_source);
        self.rev = Some(actual_rev);
        self.path_source.as_mut().unwrap().update()
    }

    fn download(&self, _: &[PackageId]) -> CargoResult<()> {
        // TODO: assert! that the PackageId is contained by the source
        Ok(())
    }

    fn get(&self, ids: &[PackageId]) -> CargoResult<Vec<Package>> {
        log!(5, "getting packages for package ids `{}` from `{}`", ids, self.remote);
        self.path_source.as_ref().expect("BUG: update() must be called before get()").get(ids)
    }

    fn fingerprint(&self, _pkg: &Package) -> CargoResult<String> {
        Ok(self.rev.as_ref().unwrap().to_string())
    }
}

#[cfg(test)]
mod test {
    use url::Url;
    use super::ident;
    use util::ToUrl;

    #[test]
    pub fn test_url_to_path_ident_with_path() {
        let ident = ident(&url("https://github.com/carlhuda/cargo"));
        assert_eq!(ident.as_slice(), "cargo-51d6ede913e3e1d5");
    }

    #[test]
    pub fn test_url_to_path_ident_without_path() {
        let ident = ident(&url("https://github.com"));
        assert_eq!(ident.as_slice(), "_empty-eba8a1ec0f6907fb");
    }

    #[test]
    fn test_canonicalize_idents_by_stripping_trailing_url_slash() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston/"));
        let ident2 = ident(&url("https://github.com/PistonDevelopers/piston"));
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_by_lowercasing_github_urls() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston"));
        let ident2 = ident(&url("https://github.com/pistondevelopers/piston"));
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_by_stripping_dot_git() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston"));
        let ident2 = ident(&url("https://github.com/PistonDevelopers/piston.git"));
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_different_protocls() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston"));
        let ident2 = ident(&url("git://github.com/PistonDevelopers/piston"));
        assert_eq!(ident1, ident2);
    }

    fn url(s: &str) -> Url {
        s.to_url().unwrap()
    }
}
