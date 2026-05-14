use std::path::Path;

use tracing::instrument;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::MaybePackage;
use crate::diagnostics::DiagnosticStats;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::rel_cwd_manifest_path;

#[instrument(skip_all)]
pub fn deferred_parse_diagnostics(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    stats: &mut DiagnosticStats,
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

    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);
    for warning in warnings {
        let msg = format!("{manifest_path}: {}", warning.message);
        if warning.is_critical {
            stats.record_error();
            gctx.shell().error(msg)?
        } else {
            stats.record_warning();
            gctx.shell().warn(msg)?
        }
    }

    Ok(())
}
