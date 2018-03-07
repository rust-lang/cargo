use super::utils::*;

pub fn cli() -> App {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg(Arg::with_name("crate"))
        .arg(
            opt("add", "Name of a user or team to add as an owner")
                .short("a").value_name("LOGIN").multiple(true)
        )
        .arg(
            opt("remove", "Name of a user or team to remove as an owner")
                .short("r").value_name("LOGIN").multiple(true)
        )
        .arg(opt("list", "List owners of a crate").short("l"))
        .arg(opt("index", "Registry index to modify owners for").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("\
    This command will modify the owners for a package
    on the specified registry(or
    default).Note that owners of a package can upload new versions, yank old
    versions.Explicitly named owners can also modify the set of owners, so take
    caution!

        See http://doc.crates.io/crates-io.html#cargo-owner for detailed documentation
        and troubleshooting.")
}
