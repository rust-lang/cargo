
use core::{SourceId, Workspace};

use util::errors::{CargoResult};

pub fn add(
    _ws: &Workspace,
    _krate: &str,
    _source_id: &SourceId,
    _vers: Option<&str>,
) -> CargoResult<()> {
    Ok(())
}
