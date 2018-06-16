
use core::{PackageId, SourceId, Workspace};

use util::errors::{CargoResult};

pub fn add(
    ws: &Workspace,
    krate: &str,
    source_id: &SourceId,
    vers: Option<&str>,
) -> CargoResult<()> {

    Ok(())
}
