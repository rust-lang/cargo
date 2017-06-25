use super::{CargoError, CargoResult};

/// Parse `flag_set_env` field in options.
pub fn parse_set_env(args: &[String]) -> CargoResult<Vec<(String, String)>> {
    let mut env_vars = Vec::new();
    for nv in args {
        let eq = nv.find('=');
        if let Some(eq) = eq {
            env_vars.push((nv[..eq].to_owned(), nv[eq+1..].to_owned()));
        } else {
            return Err(CargoError::from(
                format!("--set-env param: `{}` is is not NAME=VALUE", nv)));
        }
    }
    Ok(env_vars)
}
