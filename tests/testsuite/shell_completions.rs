use completest_pty::Runtime;

fn complete(input: &str, shell: &str) -> String {
    let shell = shell.into();

    // Load the runtime
    let mut runtime = match shell {
        "bash" => load_runtime::<completest_pty::BashRuntimeBuilder>("bash"),
        "elvish" => load_runtime::<completest_pty::ElvishRuntimeBuilder>("elvish"),
        "fish" => load_runtime::<completest_pty::FishRuntimeBuilder>("fish"),
        "zsh" => load_runtime::<completest_pty::ZshRuntimeBuilder>("zsh"),
        _ => panic!("Unsupported shell: {}", shell),
    };

    // Exec the completion
    let term = completest_pty::Term::new();
    let actual = runtime.complete(input, &term).unwrap();

    actual
}

// Return the scratch directory to keep it not being dropped
fn load_runtime<R: completest_pty::RuntimeBuilder>(shell: &str) -> Box<dyn completest_pty::Runtime>
where
    <R as completest_pty::RuntimeBuilder>::Runtime: 'static,
{
    let home = cargo_test_support::paths::home();

    let bin_path = cargo_test_support::cargo_exe();
    let bin_root = bin_path.parent().unwrap().to_owned();

    let mut runtime = Box::new(R::new(bin_root, home).unwrap());

    match shell {
        "bash" => runtime
            .register("", "source <(CARGO_COMPLETE=bash cargo)")
            .unwrap(),
        "elvish" => runtime
            .register("", "eval (E:CARGO_COMPLETE=elvish cargo | slurp)")
            .unwrap(),
        "fish" => runtime
            .register("cargo", "source (CARGO_COMPLETE=fish cargo | psub)")
            .unwrap(),
        "zsh" => runtime
            .register(
                "cargo",
                "#compdef cargo
source <(CARGO_COMPLETE=zsh cargo)",
            )
            .unwrap(),
        _ => panic!("Unsupported shell: {}", shell),
    }

    runtime
}

