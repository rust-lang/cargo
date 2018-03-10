use command_prelude::*;

pub fn cli() -> App {
    subcommand("git-checkout")
        .about("Checkout a copy of a Git repository")
        .arg(Arg::with_name("url").long("url").value_name("URL").required(true))
        .arg(Arg::with_name("reference").long("reference").value_name("REF").required(true))
}
