mod xtask;

fn main() {
    setup_logger();

    let cli = xtask::cli();
    let matches = cli.get_matches();

    let mut gctx = cargo::util::context::GlobalContext::default().unwrap_or_else(|e| {
        let mut eval = cargo::core::shell::Shell::new();
        cargo::exit_with_error(e.into(), &mut eval)
    });
    if let Err(e) = xtask::exec(&matches, &mut gctx) {
        cargo::exit_with_error(e, &mut gctx.shell())
    }
}

// In sync with `src/bin/cargo/main.rs@setup_logger`.
fn setup_logger() {
    let env = tracing_subscriber::EnvFilter::from_env("CARGO_LOG");

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::Uptime::default())
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_writer(std::io::stderr)
        .with_env_filter(env)
        .init();
}
