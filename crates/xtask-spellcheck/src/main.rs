#![allow(clippy::disallowed_methods)]
#![allow(clippy::print_stderr)]
#![allow(clippy::print_stdout)]

use anyhow::Result;
use cargo_metadata::{Metadata, MetadataCommand};
use clap::{Arg, ArgAction};
use semver::Version;
use std::{
    env, io,
    path::{Path, PathBuf},
    process::Command,
};

const BIN_NAME: &str = "typos";
const PKG_NAME: &str = "typos-cli";
const TYPOS_STEP_PREFIX: &str = "      uses: crate-ci/typos@v";

fn main() -> anyhow::Result<()> {
    let cli = cli();
    exec(&cli.get_matches())?;
    Ok(())
}

pub fn cli() -> clap::Command {
    clap::Command::new("xtask-spellcheck")
        .arg(
            Arg::new("color")
                .long("color")
                .help("Coloring: auto, always, never")
                .action(ArgAction::Set)
                .value_name("WHEN")
                .global(true),
        )
        .arg(
            Arg::new("quiet")
                .long("quiet")
                .short('q')
                .help("Do not print cargo log messages")
                .action(ArgAction::SetTrue)
                .global(true),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Use verbose output (-vv very verbose/build.rs output)")
                .action(ArgAction::Count)
                .global(true),
        )
        .arg(
            Arg::new("write-changes")
                .long("write-changes")
                .short('w')
                .help("Write fixes out")
                .action(ArgAction::SetTrue)
                .global(true),
        )
}

pub fn exec(matches: &clap::ArgMatches) -> Result<()> {
    let mut args = vec![];

    match matches.get_one::<String>("color") {
        Some(c) if matches!(c.as_str(), "auto" | "always" | "never") => {
            args.push("--color");
            args.push(c);
        }
        Some(c) => {
            anyhow::bail!(
                "argument for --color must be auto, always, or \
                 never, but found `{}`",
                c
            );
        }
        _ => {}
    }

    if matches.get_flag("quiet") {
        args.push("--quiet");
    }

    let verbose_count = matches.get_count("verbose");

    for _ in 0..verbose_count {
        args.push("--verbose");
    }
    if matches.get_flag("write-changes") {
        args.push("--write-changes");
    }

    let metadata = MetadataCommand::new()
        .exec()
        .expect("cargo_metadata failed");

    let required_version = extract_workflow_typos_version(&metadata)?;

    let outdir = metadata
        .build_directory
        .unwrap_or_else(|| metadata.target_directory)
        .as_std_path()
        .join("tmp");
    let workspace_root = metadata.workspace_root.as_path().as_std_path();
    let bin_path = crate::ensure_version_or_cargo_install(&outdir, required_version)?;

    eprintln!("running {BIN_NAME}");
    Command::new(bin_path)
        .current_dir(workspace_root)
        .args(args)
        .status()?;

    Ok(())
}

fn extract_workflow_typos_version(metadata: &Metadata) -> anyhow::Result<Version> {
    let ws_root = metadata.workspace_root.as_path().as_std_path();
    let workflow_path = ws_root.join(".github").join("workflows").join("main.yml");
    let file_content = std::fs::read_to_string(workflow_path)?;

    if let Some(line) = file_content
        .lines()
        .find(|line| line.contains(TYPOS_STEP_PREFIX))
        && let Some(stripped) = line.strip_prefix(TYPOS_STEP_PREFIX)
        && let Ok(v) = Version::parse(stripped)
    {
        Ok(v)
    } else {
        Err(anyhow::anyhow!("Could not find typos version in workflow"))
    }
}

/// If the given executable is installed with the given version, use that,
/// otherwise install via cargo.
pub fn ensure_version_or_cargo_install(
    build_dir: &Path,
    required_version: Version,
) -> io::Result<PathBuf> {
    // Check if the user has a sufficient version already installed
    let bin_path = PathBuf::from(BIN_NAME).with_extension(env::consts::EXE_EXTENSION);
    if let Some(user_version) = get_typos_version(&bin_path) {
        if user_version >= required_version {
            return Ok(bin_path);
        }
    }

    let tool_root_dir = build_dir.join("misc-tools");
    let tool_bin_dir = tool_root_dir.join("bin");
    let bin_path = tool_bin_dir
        .join(BIN_NAME)
        .with_extension(env::consts::EXE_EXTENSION);

    // Check if we have already installed sufficient version
    if let Some(misc_tools_version) = get_typos_version(&bin_path) {
        if misc_tools_version >= required_version {
            return Ok(bin_path);
        }
    }

    eprintln!("required `typos` version ({required_version}) not found, building from source");

    let mut cmd = Command::new("cargo");
    // use --force to ensure that if the required version is bumped, we update it.
    cmd.args(["install", "--locked", "--force", "--quiet"])
        .arg("--root")
        .arg(&tool_root_dir)
        // use --target-dir to ensure we have a build cache so repeated invocations aren't slow.
        .arg("--target-dir")
        .arg(tool_root_dir.join("target"))
        .arg(format!("{PKG_NAME}@{required_version}"))
        // modify PATH so that cargo doesn't print a warning telling the user to modify the path.
        .env(
            "PATH",
            env::join_paths(
                env::split_paths(&env::var("PATH").unwrap())
                    .chain(std::iter::once(tool_bin_dir.clone())),
            )
            .expect("build dir contains invalid char"),
        );

    let cargo_exit_code = cmd.spawn()?.wait()?;
    if !cargo_exit_code.success() {
        return Err(io::Error::other("cargo install failed"));
    }
    assert!(
        matches!(bin_path.try_exists(), Ok(true)),
        "cargo install did not produce the expected binary"
    );
    eprintln!("finished {BIN_NAME}");
    Ok(bin_path)
}

fn get_typos_version(bin: &PathBuf) -> Option<Version> {
    // ignore the process exit code here and instead just let the version number check fail
    if let Ok(output) = Command::new(&bin).arg("--version").output()
        && let Ok(s) = String::from_utf8(output.stdout)
        && let Some(version_str) = s.trim().split_whitespace().last()
    {
        Version::parse(version_str).ok()
    } else {
        None
    }
}
