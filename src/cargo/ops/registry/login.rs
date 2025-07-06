//! Interacts with the registry [login API][1].
//!
//! This doesn't really call any web API at this moment. Instead, it's just an
//! operation for `cargo login`.
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#login

use std::io::IsTerminal;

use crate::CargoResult;
use crate::GlobalContext;
use crate::util::auth;
use crate::util::auth::AuthorizationError;
use cargo_credential::LoginOptions;
use cargo_credential::Secret;

use super::RegistryOrIndex;
use super::get_source_id;
use super::registry;

pub fn registry_login(
    gctx: &GlobalContext,
    token_from_cmdline: Option<Secret<&str>>,
    reg_or_index: Option<&RegistryOrIndex>,
    args: &[&str],
) -> CargoResult<()> {
    let source_ids = get_source_id(gctx, reg_or_index)?;

    let login_url = match registry(
        gctx,
        &source_ids,
        token_from_cmdline.clone(),
        reg_or_index,
        false,
        None,
    ) {
        Ok((registry, _)) => Some(format!("{}/me", registry.host())),
        Err(e) if e.is::<AuthorizationError>() => e
            .downcast::<AuthorizationError>()
            .unwrap()
            .login_url
            .map(|u| u.to_string()),
        Err(e) => return Err(e),
    };

    let mut token_from_stdin = None;
    let token = token_from_cmdline.or_else(|| {
        if !std::io::stdin().is_terminal() {
            let token = cargo_credential::read_line().unwrap_or_default();
            if !token.is_empty() {
                token_from_stdin = Some(token);
            }
        }
        token_from_stdin.as_deref().map(Secret::from)
    });

    let options = LoginOptions {
        token,
        login_url: login_url.as_deref(),
    };

    auth::login(gctx, &source_ids.original, options, args)?;
    Ok(())
}
