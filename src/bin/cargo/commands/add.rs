use command_prelude::*;

use cargo::core::{SourceId};

pub fn cli() -> App {
    subcommand("add")
        .about("Add a new dependency")
        .arg(Arg::with_name("crate").empty_values(false))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    println!("cargo add subcommand executed");

    let krate = args.value_of("crate")
        .unwrap_or_default();

    println!("crate {:?}", krate);

    // let source = SourceId::crates_io(config)?;

    Ok(())
}
