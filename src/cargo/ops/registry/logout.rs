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
    let reg_cfg = auth::registry_credential_config(config, &source_ids.original)?;
    let reg_name = source_ids.original.display_registry_name();
    if reg_cfg.is_none() {
        config
            .shell()
            .status("Logout", format!("not currently logged in to `{reg_name}`"))?;
        return Ok(());
    }
    auth::logout(config, &source_ids.original)?;
    config.shell().status(
        "Logout",
        format!("token for `{reg_name}` has been removed from local storage"),
    )?;
    let location = if source_ids.original.is_crates_io() {
        "<https://crates.io/me>".to_string()
    } else {
        // The URL for the source requires network access to load the config.
        // That could be a fairly heavy operation to perform just to provide a
        // help message, so for now this just provides some generic text.
        // Perhaps in the future this could have an API to fetch the config if
        // it is cached, but avoid network access otherwise?
        format!("the `{reg_name}` website")
    };
    config.shell().note(format!(
        "This does not revoke the token on the registry server.\n    \
        If you need to revoke the token, visit {location} and follow the instructions there."
    ))?;
    Ok(())
}
