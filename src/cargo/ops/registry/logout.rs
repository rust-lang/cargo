//! Interacts with the registry logout.
//!
//! There is no web API for logout at this moment. Instead, it's just an
//! operation for `cargo logout`.

use crate::CargoResult;
use crate::GlobalContext;
use crate::util::auth;

use super::RegistryOrIndex;
use super::get_source_id;

pub fn registry_logout(
    gctx: &GlobalContext,
    reg_or_index: Option<RegistryOrIndex>,
) -> CargoResult<()> {
    let source_ids = get_source_id(gctx, reg_or_index.as_ref())?;
    auth::logout(gctx, &source_ids.original)?;
    Ok(())
}
