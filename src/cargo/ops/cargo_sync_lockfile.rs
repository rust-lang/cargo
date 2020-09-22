use crate::core::{PackageSet, Resolve, Workspace};
use crate::ops;
use crate::util::CargoResult;
use crate::util::Config;

pub struct SyncLockfileOptions<'a> {
    pub config: &'a Config,
    /// The target arch triple to sync lockfile dependencies for
    pub targets: Vec<String>,
}

/// Executes `cargo sync_lockfile`.
pub fn sync_lockfile<'a>(
    ws: &Workspace<'a>,
    _options: &SyncLockfileOptions<'a>,
) -> CargoResult<(Resolve, PackageSet<'a>)> {
    ws.emit_warnings()?;
    let (packages, resolve) = ops::resolve_ws(ws)?;
    Ok((resolve, packages))
}
