use crate::command_prelude::*;

use std::cmp::min;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("search")
        .about("Search packages in crates.io")
        .arg(Arg::new("query").value_name("QUERY").num_args(0..))
        .arg(
            opt(
                "limit",
                "Limit the number of results (default: 10, max: 100)",
            )
            .value_name("LIMIT"),
        )
        .arg_index("Registry index URL to search packages in")
        .arg_registry("Registry to search packages in")
        .arg_quiet()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help search</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let reg_or_index = args.registry_or_index(config)?;
    let limit = args.value_of_u32("limit")?;
    let limit = min(100, limit.unwrap_or(10));
    let query: Vec<&str> = args
        .get_many::<String>("query")
        .unwrap_or_default()
        .map(String::as_str)
        .collect();
    let query: String = query.join("+");
    ops::search(&query, config, reg_or_index, limit)?;
    Ok(())
}
