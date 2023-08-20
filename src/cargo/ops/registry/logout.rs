//! Interacts with the registry logout.
//!
//! There is no web API for logout at this moment. Instead, it's just an
//! operation for `cargo logout`.

use crate::util::auth;
use crate::CargoResult;
use crate::Config;

use super::get_source_id;

pub fn registry_logout(config: &Config, reg: Option<&str>) -> CargoResult<()> {
    let source_ids = get_source_id(config, None, reg)?;
    auth::logout(config, &source_ids.original)?;
    Ok(())
}
