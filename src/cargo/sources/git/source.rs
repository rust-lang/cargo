use std::fmt::{self, Debug, Formatter};

use log::trace;
use url::Url;

use crate::core::source::{MaybePackage, Source, SourceId};
use crate::core::GitReference;
use crate::core::{Dependency, Package, PackageId, Summary};
use crate::sources::git::utils::{GitRemote, GitRevision};
use crate::sources::PathSource;
use crate::util::errors::CargoResult;
use crate::util::hex::short_hash;
use crate::util::Config;

pub struct GitSource<'cfg> {
    remote: GitRemote,
    reference: GitReference,
    source_id: SourceId,
    path_source: Option<PathSource<'cfg>>,
    rev: Option<GitRevision>,
    ident: String,
    config: &'cfg Config,
}

impl<'cfg> GitSource<'cfg> {
    pub fn new(source_id: SourceId, config: &'cfg Config) -> CargoResult<GitSource<'cfg>> {
        assert!(source_id.is_git(), "id is not git, id={}", source_id);

        let remote = GitRemote::new(source_id.url());
        let ident = ident(source_id.url())?;

        let reference = match source_id.precise() {
            Some(s) => GitReference::Rev(s.to_string()),
            None => source_id.git_reference().unwrap().clone(),
        };

        let source = GitSource {
            remote,
            reference,
            source_id,
            path_source: None,
            rev: None,
            ident,
            config,
        };

        Ok(source)
    }

    pub fn url(&self) -> &Url {
        self.remote.url()
    }

    pub fn read_packages(&mut self) -> CargoResult<Vec<Package>> {
        if self.path_source.is_none() {
            self.update()?;
        }
        self.path_source.as_mut().unwrap().read_packages()
    }
}

fn ident(url: &Url) -> CargoResult<String> {
    let url = canonicalize_url(url)?;
    let ident = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("");

    let ident = if ident == "" { "_empty" } else { ident };

    Ok(format!("{}-{}", ident, short_hash(&url)))
}

// Some hacks and heuristics for making equivalent URLs hash the same.
pub fn canonicalize_url(url: &Url) -> CargoResult<Url> {
    let mut url = url.clone();

    // cannot-be-a-base-urls (e.g., `github.com:rust-lang-nursery/rustfmt.git`)
    // are not supported.
    if url.cannot_be_a_base() {
        failure::bail!(
            "invalid url `{}`: cannot-be-a-base-URLs are not supported",
            url
        )
    }

    // Strip a trailing slash.
    if url.path().ends_with('/') {
        url.path_segments_mut().unwrap().pop_if_empty();
    }

    // HACK: for GitHub URLs specifically, just lower-case
    // everything. GitHub treats both the same, but they hash
    // differently, and we're gonna be hashing them. This wants a more
    // general solution, and also we're almost certainly not using the
    // same case conversion rules that GitHub does. (See issue #84.)
    if url.host_str() == Some("github.com") {
        url.set_scheme("https").unwrap();
        let path = url.path().to_lowercase();
        url.set_path(&path);
    }

    // Repos can generally be accessed with or without `.git` extension.
    let needs_chopping = url.path().ends_with(".git");
    if needs_chopping {
        let last = {
            let last = url.path_segments().unwrap().next_back().unwrap();
            last[..last.len() - 4].to_owned()
        };
        url.path_segments_mut().unwrap().pop().push(&last);
    }

    Ok(url)
}

impl<'cfg> Debug for GitSource<'cfg> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "git repo at {}", self.remote.url())?;

        match self.reference.pretty_ref() {
            Some(s) => write!(f, " ({})", s),
            None => Ok(()),
        }
    }
}

impl<'cfg> Source for GitSource<'cfg> {
    fn query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        let src = self
            .path_source
            .as_mut()
            .expect("BUG: `update()` must be called before `query()`");
        src.query(dep, f)
    }

    fn fuzzy_query(&mut self, dep: &Dependency, f: &mut dyn FnMut(Summary)) -> CargoResult<()> {
        let src = self
            .path_source
            .as_mut()
            .expect("BUG: `update()` must be called before `query()`");
        src.fuzzy_query(dep, f)
    }

    fn supports_checksums(&self) -> bool {
        false
    }

    fn requires_precise(&self) -> bool {
        true
    }

    fn source_id(&self) -> SourceId {
        self.source_id
    }

    fn update(&mut self) -> CargoResult<()> {
        let lock =
            self.config
                .git_path()
                .open_rw(".cargo-lock-git", self.config, "the git checkouts")?;

        let db_path = lock.parent().join("db").join(&self.ident);

        if self.config.cli_unstable().offline && !db_path.exists() {
            failure::bail!(
                "can't checkout from '{}': you are in the offline mode (-Z offline)",
                self.remote.url()
            );
        }

        // Resolve our reference to an actual revision, and check if the
        // database already has that revision. If it does, we just load a
        // database pinned at that revision, and if we don't we issue an update
        // to try to find the revision.
        let actual_rev = self.remote.rev_for(&db_path, &self.reference);
        let should_update = actual_rev.is_err() || self.source_id.precise().is_none();

        let (db, actual_rev) = if should_update && !self.config.cli_unstable().offline {
            self.config.shell().status(
                "Updating",
                format!("git repository `{}`", self.remote.url()),
            )?;

            trace!("updating git source `{:?}`", self.remote);

            self.remote
                .checkout(&db_path, &self.reference, self.config)?
        } else {
            (self.remote.db_at(&db_path)?, actual_rev.unwrap())
        };

        // Don’t use the full hash, in order to contribute less to reaching the path length limit
        // on Windows. See <https://github.com/servo/servo/pull/14397>.
        let short_id = db.to_short_id(&actual_rev).unwrap();

        let checkout_path = lock
            .parent()
            .join("checkouts")
            .join(&self.ident)
            .join(short_id.as_str());

        // Copy the database to the checkout location. After this we could drop
        // the lock on the database as we no longer needed it, but we leave it
        // in scope so the destructors here won't tamper with too much.
        // Checkout is immutable, so we don't need to protect it with a lock once
        // it is created.
        db.copy_to(actual_rev.clone(), &checkout_path, self.config)?;

        let source_id = self.source_id.with_precise(Some(actual_rev.to_string()));
        let path_source = PathSource::new_recursive(&checkout_path, source_id, self.config);

        self.path_source = Some(path_source);
        self.rev = Some(actual_rev);
        self.path_source.as_mut().unwrap().update()
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        trace!(
            "getting packages for package ID `{}` from `{:?}`",
            id,
            self.remote
        );
        self.path_source
            .as_mut()
            .expect("BUG: `update()` must be called before `get()`")
            .download(id)
    }

    fn finish_download(&mut self, _id: PackageId, _data: Vec<u8>) -> CargoResult<Package> {
        panic!("no download should have started")
    }

    fn fingerprint(&self, _pkg: &Package) -> CargoResult<String> {
        Ok(self.rev.as_ref().unwrap().to_string())
    }

    fn describe(&self) -> String {
        format!("Git repository {}", self.source_id)
    }

    fn add_to_yanked_whitelist(&mut self, _pkgs: &[PackageId]) {}
}

#[cfg(test)]
mod test {
    use super::ident;
    use crate::util::ToUrl;
    use url::Url;

    #[test]
    pub fn test_url_to_path_ident_with_path() {
        let ident = ident(&url("https://github.com/carlhuda/cargo")).unwrap();
        assert!(ident.starts_with("cargo-"));
    }

    #[test]
    pub fn test_url_to_path_ident_without_path() {
        let ident = ident(&url("https://github.com")).unwrap();
        assert!(ident.starts_with("_empty-"));
    }

    #[test]
    fn test_canonicalize_idents_by_stripping_trailing_url_slash() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston/")).unwrap();
        let ident2 = ident(&url("https://github.com/PistonDevelopers/piston")).unwrap();
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_by_lowercasing_github_urls() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston")).unwrap();
        let ident2 = ident(&url("https://github.com/pistondevelopers/piston")).unwrap();
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_by_stripping_dot_git() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston")).unwrap();
        let ident2 = ident(&url("https://github.com/PistonDevelopers/piston.git")).unwrap();
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_different_protocols() {
        let ident1 = ident(&url("https://github.com/PistonDevelopers/piston")).unwrap();
        let ident2 = ident(&url("git://github.com/PistonDevelopers/piston")).unwrap();
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_cannot_be_a_base_urls() {
        assert!(ident(&url("github.com:PistonDevelopers/piston")).is_err());
        assert!(ident(&url("google.com:PistonDevelopers/piston")).is_err());
    }

    fn url(s: &str) -> Url {
        s.to_url().unwrap()
    }
}
