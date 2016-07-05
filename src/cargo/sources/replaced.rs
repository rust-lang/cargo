use core::{Source, Registry, PackageId, Package, Dependency, Summary, SourceId};
use util::{CargoResult, ChainError, human};

pub struct ReplacedSource<'cfg> {
    to_replace: SourceId,
    replace_with: SourceId,
    inner: Box<Source + 'cfg>,
}

impl<'cfg> ReplacedSource<'cfg> {
    pub fn new(to_replace: &SourceId,
               replace_with: &SourceId,
               src: Box<Source + 'cfg>) -> ReplacedSource<'cfg> {
        ReplacedSource {
            to_replace: to_replace.clone(),
            replace_with: replace_with.clone(),
            inner: src,
        }
    }
}

impl<'cfg> Registry for ReplacedSource<'cfg> {
    fn query(&mut self, dep: &Dependency) -> CargoResult<Vec<Summary>> {
        let dep = dep.clone().map_source(&self.to_replace, &self.replace_with);
        let ret = try!(self.inner.query(&dep).chain_error(|| {
            human(format!("failed to query replaced source `{}`",
                          self.to_replace))
        }));
        Ok(ret.into_iter().map(|summary| {
            summary.map_source(&self.replace_with, &self.to_replace)
        }).collect())
    }
}

impl<'cfg> Source for ReplacedSource<'cfg> {
    fn update(&mut self) -> CargoResult<()> {
        self.inner.update().chain_error(|| {
            human(format!("failed to update replaced source `{}`",
                          self.to_replace))
        })
    }

    fn download(&mut self, id: &PackageId) -> CargoResult<Package> {
        let id = id.with_source_id(&self.replace_with);
        let pkg = try!(self.inner.download(&id).chain_error(|| {
            human(format!("failed to download replaced source `{}`",
                          self.to_replace))
        }));
        Ok(pkg.map_source(&self.replace_with, &self.to_replace))
    }

    fn fingerprint(&self, id: &Package) -> CargoResult<String> {
        self.inner.fingerprint(&id)
    }

    fn verify(&self, id: &PackageId) -> CargoResult<()> {
        let id = id.with_source_id(&self.replace_with);
        self.inner.verify(&id)
    }
}
