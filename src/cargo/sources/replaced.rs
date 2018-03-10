use core::{Source, Registry, PackageId, Package, Dependency, Summary, SourceId};
use util::errors::{CargoResult, CargoResultExt};

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
    fn query(&mut self,
             dep: &Dependency,
             f: &mut FnMut(Summary)) -> CargoResult<()> {
        let (replace_with, to_replace) = (&self.replace_with, &self.to_replace);
        let dep = dep.clone().map_source(to_replace, replace_with);

        self.inner.query(&dep, &mut |summary| {
            f(summary.map_source(replace_with, to_replace))
        }).chain_err(|| {
            format!("failed to query replaced source {}",
                    self.to_replace)
        })?;
        Ok(())
    }

    fn supports_checksums(&self) -> bool {
        self.inner.supports_checksums()
    }

    fn requires_precise(&self) -> bool {
        self.inner.requires_precise()
    }
}

impl<'cfg> Source for ReplacedSource<'cfg> {
    fn source_id(&self) -> &SourceId {
        &self.to_replace
    }

    fn update(&mut self) -> CargoResult<()> {
        self.inner.update().chain_err(|| {
            format!("failed to update replaced source {}",
                    self.to_replace)
        })?;
        Ok(())
    }

    fn download(&mut self, ids: &[&PackageId]) -> CargoResult<Vec<Package>> {
        let replaced_ids: Vec<_> = ids.iter().map(|id| id.with_source_id(&self.replace_with)).collect();
        let replaced_id_refs: Vec<_> = replaced_ids.iter().collect();
        let pkgs = self.inner.download(&*replaced_id_refs).chain_err(|| {
            format!("failed to download replaced source {}",
                    self.to_replace)
        })?;
        Ok(pkgs.into_iter().map(|x| x.map_source(&self.replace_with, &self.to_replace)).collect())
    }

    fn fingerprint(&self, id: &Package) -> CargoResult<String> {
        self.inner.fingerprint(id)
    }

    fn verify(&self, id: &PackageId) -> CargoResult<()> {
        let id = id.with_source_id(&self.replace_with);
        self.inner.verify(&id)
    }
}
