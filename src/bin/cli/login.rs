use super::command_prelude::*;

pub fn cli() -> App {
    subcommand("login")
        .about("Save an api token from the registry locally. \
                If token is not specified, it will be read from stdin.")
        .arg(Arg::with_name("token"))
        .arg(opt("host", "Host to set the token for").value_name("HOST"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
}
