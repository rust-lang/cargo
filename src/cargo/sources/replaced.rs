use crate::core::{Dependency, Package, PackageId, SourceId};
use crate::sources::source::MaybePackage;
use crate::sources::source::QueryKind;
use crate::sources::source::Source;
use crate::sources::IndexSummary;
use crate::util::errors::CargoResult;
use std::task::Poll;

/// A source that replaces one source with the other. This manages the [source
/// replacement] feature.
///
/// The implementation is merely redirecting from the original to the replacement.
///
/// [source replacement]: https://doc.rust-lang.org/nightly/cargo/reference/source-replacement.html
pub struct ReplacedSource<'gctx> {
    /// The identifier of the original source.
    to_replace: SourceId,
    /// The identifier of the new replacement source.
    replace_with: SourceId,
    inner: Box<dyn Source + 'gctx>,
}

impl<'gctx> ReplacedSource<'gctx> {
    /// Creates a replaced source.
    ///
    /// The `src` argument is the new replacement source.
    pub fn new(
        to_replace: SourceId,
        replace_with: SourceId,
        src: Box<dyn Source + 'gctx>,
    ) -> ReplacedSource<'gctx> {
        ReplacedSource {
            to_replace,
            replace_with,
            inner: src,
        }
    }

    /// Is this source a built-in replacement of crates.io?
    ///
    /// Built-in source replacement of crates.io for sparse registry or tests
    /// should not show messages indicating source replacement.
    fn is_builtin_replacement(&self) -> bool {
        self.replace_with.is_crates_io() && self.to_replace.is_crates_io()
    }
}

impl<'gctx> Source for ReplacedSource<'gctx> {
    fn source_id(&self) -> SourceId {
        self.to_replace
    }

    fn replaced_source_id(&self) -> SourceId {
        self.replace_with
    }

    fn supports_checksums(&self) -> bool {
        self.inner.supports_checksums()
    }

    fn requires_precise(&self) -> bool {
        self.inner.requires_precise()
    }

    fn query(
        &mut self,
        dep: &Dependency,
        kind: QueryKind,
        f: &mut dyn FnMut(IndexSummary),
    ) -> Poll<CargoResult<()>> {
        let (replace_with, to_replace) = (self.replace_with, self.to_replace);
        let dep = dep.clone().map_source(to_replace, replace_with);

        self.inner
            .query(&dep, kind, &mut |summary| {
                f(summary.map_summary(|s| s.map_source(replace_with, to_replace)))
            })
            .map_err(|e| {
                if self.is_builtin_replacement() {
                    e
                } else {
                    e.context(format!(
                        "failed to query replaced source {}",
                        self.to_replace
                    ))
                }
            })
    }

    fn invalidate_cache(&mut self) {
        self.inner.invalidate_cache()
    }

    fn set_quiet(&mut self, quiet: bool) {
        self.inner.set_quiet(quiet);
    }

    fn download(&mut self, id: PackageId) -> CargoResult<MaybePackage> {
        let id = id.with_source_id(self.replace_with);
        let pkg = self.inner.download(id).map_err(|e| {
            if self.is_builtin_replacement() {
                e
            } else {
                e.context(format!(
                    "failed to download replaced source {}",
                    self.to_replace
                ))
            }
        })?;
        Ok(match pkg {
            MaybePackage::Ready(pkg) => {
                MaybePackage::Ready(pkg.map_source(self.replace_with, self.to_replace))
            }
            other @ MaybePackage::Download { .. } => other,
        })
    }

    fn finish_download(&mut self, id: PackageId, data: Vec<u8>) -> CargoResult<Package> {
        let id = id.with_source_id(self.replace_with);
        let pkg = self.inner.finish_download(id, data).map_err(|e| {
            if self.is_builtin_replacement() {
                e
            } else {
                e.context(format!(
                    "failed to download replaced source {}",
                    self.to_replace
                ))
            }
        })?;
        Ok(pkg.map_source(self.replace_with, self.to_replace))
    }

    fn fingerprint(&self, id: &Package) -> CargoResult<String> {
        self.inner.fingerprint(id)
    }

    fn verify(&self, id: PackageId) -> CargoResult<()> {
        let id = id.with_source_id(self.replace_with);
        self.inner.verify(id)
    }

    fn describe(&self) -> String {
        if self.is_builtin_replacement() {
            self.inner.describe()
        } else {
            format!(
                "{} (which is replacing {})",
                self.inner.describe(),
                self.to_replace
            )
        }
    }

    fn is_replaced(&self) -> bool {
        !self.is_builtin_replacement()
    }

    fn add_to_yanked_whitelist(&mut self, pkgs: &[PackageId]) {
        let pkgs = pkgs
            .iter()
            .map(|id| id.with_source_id(self.replace_with))
            .collect::<Vec<_>>();
        self.inner.add_to_yanked_whitelist(&pkgs);
    }

    fn is_yanked(&mut self, pkg: PackageId) -> Poll<CargoResult<bool>> {
        self.inner.is_yanked(pkg)
    }

    fn block_until_ready(&mut self) -> CargoResult<()> {
        self.inner.block_until_ready().map_err(|e| {
            if self.is_builtin_replacement() {
                e
            } else {
                e.context(format!(
                    "failed to update replaced source {}",
                    self.to_replace
                ))
            }
        })
    }
}
