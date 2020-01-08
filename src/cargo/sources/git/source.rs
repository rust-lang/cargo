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
        let ident = ident(&source_id);

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

fn ident(id: &SourceId) -> String {
    let ident = id
        .canonical_url()
        .raw_canonicalized_url()
        .path_segments()
        .and_then(|s| s.rev().next())
        .unwrap_or("");

    let ident = if ident == "" { "_empty" } else { ident };

    format!("{}-{}", ident, short_hash(id.canonical_url()))
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
        let git_path = self.config.git_path();
        let git_path = self.config.assert_package_cache_locked(&git_path);
        let db_path = git_path.join("db").join(&self.ident);

        if self.config.offline() && !db_path.exists() {
            anyhow::bail!(
                "can't checkout from '{}': you are in the offline mode (--offline)",
                self.remote.url()
            );
        }

        // Resolve our reference to an actual revision, and check if the
        // database already has that revision. If it does, we just load a
        // database pinned at that revision, and if we don't we issue an update
        // to try to find the revision.
        let actual_rev = self.remote.rev_for(&db_path, &self.reference);
        let should_update = actual_rev.is_err() || self.source_id.precise().is_none();

        let (db, actual_rev) = if should_update && !self.config.offline() {
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

        // Don’t use the full hash, in order to contribute less to reaching the
        // path length limit on Windows. See
        // <https://github.com/servo/servo/pull/14397>.
        let short_id = db.to_short_id(&actual_rev).unwrap();

        let checkout_path = git_path
            .join("checkouts")
            .join(&self.ident)
            .join(short_id.as_str());

        // Copy the database to the checkout location.
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

    fn is_yanked(&mut self, _pkg: PackageId) -> CargoResult<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod test {
    use super::ident;
    use crate::core::{GitReference, SourceId};
    use crate::util::IntoUrl;

    #[test]
    pub fn test_url_to_path_ident_with_path() {
        let ident = ident(&src("https://github.com/carlhuda/cargo"));
        assert!(ident.starts_with("cargo-"));
    }

    #[test]
    pub fn test_url_to_path_ident_without_path() {
        let ident = ident(&src("https://github.com"));
        assert!(ident.starts_with("_empty-"));
    }

    #[test]
    fn test_canonicalize_idents_by_stripping_trailing_url_slash() {
        let ident1 = ident(&src("https://github.com/PistonDevelopers/piston/"));
        let ident2 = ident(&src("https://github.com/PistonDevelopers/piston"));
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_by_lowercasing_github_urls() {
        let ident1 = ident(&src("https://github.com/PistonDevelopers/piston"));
        let ident2 = ident(&src("https://github.com/pistondevelopers/piston"));
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_by_stripping_dot_git() {
        let ident1 = ident(&src("https://github.com/PistonDevelopers/piston"));
        let ident2 = ident(&src("https://github.com/PistonDevelopers/piston.git"));
        assert_eq!(ident1, ident2);
    }

    #[test]
    fn test_canonicalize_idents_different_protocols() {
        let ident1 = ident(&src("https://github.com/PistonDevelopers/piston"));
        let ident2 = ident(&src("git://github.com/PistonDevelopers/piston"));
        assert_eq!(ident1, ident2);
    }

    fn src(s: &str) -> SourceId {
        SourceId::for_git(
            &s.into_url().unwrap(),
            GitReference::Branch("master".to_string()),
        )
        .unwrap()
    }
}
