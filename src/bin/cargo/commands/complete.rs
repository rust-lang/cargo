use crate::command_prelude::*;

use clap::Shell;

pub fn cli() -> App {
    subcommand("complete")
        .about("Generate completion file for a shell")
        .arg(
            Arg::with_name("shell")
                .takes_value(true)
                .required(true)
                .help("The shell to generate a completion file for")
        )
        .after_help("Run `cargo help complete` for more detailed information.\n")
}

pub fn exec(_config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let shell_name = args.value_of("shell").unwrap();
    let shell: Shell = shell_name.parse()
        // TODO - proper error handling
        .expect(&format!("unknown shell: {}", shell_name));

    crate::cli::cli().gen_completions_to("cargo", shell, &mut std::io::stdout());

    Ok(())
}
