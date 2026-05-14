use std::path::Path;

use tracing::instrument;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::diagnostics::ManifestFor;

#[instrument(skip_all)]
pub fn deferred_parse_diagnostics(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let warnings = match &manifest {
        ManifestFor::Package(pkg) => pkg.manifest().warnings().warnings(),
        ManifestFor::Workspace {
            maybe_pkg: MaybePackage::Virtual(vm),
            ..
        } => vm.warnings().warnings(),
        ManifestFor::Workspace {
            maybe_pkg: MaybePackage::Package(_),
            ..
        } => {
            // For real manifests, lint as a package, rather than a workspace
            return Ok(());
        }
    };

    for warning in warnings {
        if warning.is_critical {
            let err = anyhow::format_err!("{}", warning.message);
            let cx =
                anyhow::format_err!("failed to parse manifest at `{}`", manifest_path.display());
            return Err(err.context(cx));
        } else {
            let msg = format!("{}: {}", manifest_path.display(), warning.message);

            gctx.shell().warn(msg)?
        }
    }

    Ok(())
}
