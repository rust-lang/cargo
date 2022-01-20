use crate::command_prelude::*;

use std::cmp::min;

use cargo::ops;

pub fn cli() -> App {
    subcommand("search")
        .about("Search packages in crates.io")
        .arg_quiet()
        .arg(Arg::new("query").multiple_values(true))
        .arg_index()
        .arg(
            opt(
                "limit",
                "Limit the number of results (default: 10, max: 100)",
            )
            .value_name("LIMIT"),
        )
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help search` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;
    let index = args.index()?;
    let limit = args.value_of_u32("limit")?;
    let limit = min(100, limit.unwrap_or(10));
    let query: Vec<&str> = args.values_of("query").unwrap_or_default().collect();
    let query: String = query.join("+");
    ops::search(&query, config, index, limit, registry)?;
    Ok(())
}
