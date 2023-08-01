mod xtask;

fn main() {
    env_logger::init_from_env("CARGO_LOG");
    let cli = xtask::cli();
    let matches = cli.get_matches();

    let mut config = cargo::util::config::Config::default().unwrap_or_else(|e| {
        let mut eval = cargo::core::shell::Shell::new();
        cargo::exit_with_error(e.into(), &mut eval)
    });
    if let Err(e) = xtask::exec(&matches, &mut config) {
        cargo::exit_with_error(e, &mut config.shell())
    }
}
