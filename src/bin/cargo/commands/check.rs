use cargo::ops;

use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("check")
        // subcommand aliases are handled in aliased_command()
        // .alias("c")
        .about("Check a local package and all of its dependencies for errors")
        .arg_future_incompat_report()
        .arg_message_format()
        .arg_silent_suggestion()
        .arg_package_spec(
            "Package(s) to check",
            "Check all packages in the workspace",
            "Exclude packages from the check",
        )
        .arg_targets_all(
            "Check only this package's library",
            "Check only the specified binary",
            "Check all binaries",
            "Check only the specified example",
            "Check all examples",
            "Check only the specified test target",
            "Check all targets that have `test = true` set",
            "Check only the specified bench target",
            "Check all targets that have `bench = true` set",
            "Check all targets",
        )
        .arg_features()
        .arg_parallel()
        .arg_release("Check artifacts in release mode, with optimizations")
        .arg_profile("Check artifacts with the specified profile")
        .arg_target_triple("Check for the target triple")
        .arg_target_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_manifest_path()
        .arg_lockfile_path()
        .arg_ignore_rust_version()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help check</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    if std::env::var("CARGO_REAL_CHECK").is_err() {
        fixit()?;
        return Ok(());
    }
    let ws = args.workspace(gctx)?;
    // This is a legacy behavior that causes `cargo check` to pass `--test`.
    let test = matches!(args.get_one::<String>("profile").map(String::as_str), Some("test"));
    let mode = CompileMode::Check { test };
    let compile_opts =
        args.compile_options(gctx, mode, Some(&ws), ProfileChecking::LegacyTestOnly)?;

    ops::compile(&ws, &compile_opts)?;
    Ok(())
}

fn fixit() -> CliResult {
    use std::path::Path;

    use anyhow::Context;
    use cargo_util::{paths, ProcessBuilder};

    eprintln!("Copying to /tmp/fixit");
    ProcessBuilder::new("cp").args(&["-a", ".", "/tmp/fixit"]).exec()?;
    std::env::set_current_dir("/tmp/fixit").map_err(|e| anyhow::format_err!("cd failed {}", e))?;

    let ed_re = regex::Regex::new(r#"(?m)^ *edition *= *['"]([^'"]+)['"]"#).unwrap();
    let manifest = paths::read(Path::new("Cargo.toml"))?;
    let ed_cap = match ed_re.captures(&manifest) {
        None => {
            eprintln!("no edition found in manifest, probably 2015, skipping");
            return Ok(());
        }
        Some(caps) => caps.get(1).unwrap(),
    };
    if ed_cap.as_str() != "2021" {
        eprintln!("skipping non-2021 edition `{}`", ed_cap.as_str());
        return Ok(());
    }
    eprintln!("Running `cargo fix --edition`");
    // Skip "cargo check"
    let args: Vec<_> = std::env::args().skip(2).collect();
    ProcessBuilder::new("cargo")
        .args(&["fix", "--edition", "--allow-no-vcs", "--allow-dirty"])
        .args(&args)
        .exec()
        .with_context(|| "failed to migrate to next edition")?;
    let mut manifest = paths::read(Path::new("Cargo.toml"))?;
    let ed_cap = ed_re.captures(&manifest).unwrap().get(1).unwrap();
    manifest.replace_range(ed_cap.range(), "2024");
    paths::write("Cargo.toml", manifest)?;
    eprintln!("Running `cargo check` to verify 2024");
    ProcessBuilder::new("cargo")
        .args(&["check"])
        .args(&args)
        .env("CARGO_REAL_CHECK", "1")
        .exec()
        .with_context(|| "failed to check after updating to 2024")?;
    Ok(())
}
