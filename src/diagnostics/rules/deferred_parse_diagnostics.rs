use std::path::Path;

use tracing::instrument;

use crate::CargoResult;
use crate::GlobalContext;
use crate::diagnostics::ManifestFor;
use crate::diagnostics::ScopedDiagnosticStats;
use crate::diagnostics::workspace_rel_path;
use crate::workspace::MaybePackage;
use crate::workspace::Workspace;

#[instrument(skip_all)]
pub(crate) fn diagnose_manifest(
    ws: &Workspace<'_>,
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    pkg_stats: &mut ScopedDiagnosticStats<'_>,
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

    let manifest_path = workspace_rel_path(ws, manifest_path);
    for warning in warnings {
        let msg = format!("{manifest_path}: {}", warning.message);
        if warning.is_critical {
            pkg_stats.record_error();
            gctx.shell().error(msg)?
        } else {
            pkg_stats.record_warning();
            gctx.shell().warn(msg)?
        }
    }

    Ok(())
}
