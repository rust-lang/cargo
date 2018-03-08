use super::utils::*;

pub fn cli() -> App {
    subcommand("search")
        .about("Search packages in crates.io")
        .arg(Arg::with_name("query").multiple(true))
        .arg_index()
        .arg(
            opt("limit", "Limit the number of results (default: 10, max: 100)")
                .value_name("LIMIT")
        )
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
}
