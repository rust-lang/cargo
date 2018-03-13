use command_prelude::*;

pub fn cli() -> App {
    subcommand("dirs")
        .about("Display directories (cache, config, ...) used by cargo")
        .after_help("\
")
}

pub fn exec(config: &mut Config, _args: &ArgMatches) -> CliResult {
    println!("CARGO_CACHE_DIR:  {:?}", config.cache_path().into_path_unlocked());
    println!("CARGO_CONFIG_DIR: {:?}", config.config_path().into_path_unlocked());
    println!("CARGO_DATA_DIR:   {:?}", config.data_path());
    println!("CARGO_BIN_DIR:    {:?}", config.bin_path());
    Ok(())
}
