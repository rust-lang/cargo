use std::path::Path;

use crate::core::{EitherManifest, Package, SourceId};
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::toml::read_manifest;
use tracing::trace;

pub fn read_package(
    path: &Path,
    source_id: SourceId,
    gctx: &GlobalContext,
) -> CargoResult<Package> {
    trace!(
        "read_package; path={}; source-id={}",
        path.display(),
        source_id
    );
    let manifest = read_manifest(path, source_id, gctx)?;
    let manifest = match manifest {
        EitherManifest::Real(manifest) => manifest,
        EitherManifest::Virtual(..) => anyhow::bail!(
            "found a virtual manifest at `{}` instead of a package \
             manifest",
            path.display()
        ),
    };

    Ok(Package::new(manifest, path))
}
