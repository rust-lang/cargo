use build_rs::{input, output};
use std::path::Path;
use std::process::Command;

fn main() {
    commit_info();
    windows_manifest();
    let target = input::target();
    output::rustc_env("RUST_HOST_TARGET", &target);
}

struct CommitInfo {
    hash: String,
    short_hash: String,
    date: String,
}

fn commit_info_from_git() -> Option<CommitInfo> {
    if !Path::new(".git").exists() {
        return None;
    }

    let output = match Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--date=short")
        .arg("--format=%H %h %cd")
        .arg("--abbrev=9")
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return None,
    };

    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut parts = stdout.split_whitespace().map(|s| s.to_string());

    Some(CommitInfo {
        hash: parts.next()?,
        short_hash: parts.next()?,
        date: parts.next()?,
    })
}

// The rustc source tarball is meant to contain all the source code to build an exact copy of the
// toolchain, but it doesn't include the git repository itself. It wouldn't thus be possible to
// populate the version information with the commit hash and the commit date.
//
// To work around this, the rustc build process obtains the git information when creating the
// source tarball and writes it to the `git-commit-info` file. The build process actually creates
// at least *two* of those files, one for Rust as a whole (in the root of the tarball) and one
// specifically for Cargo (in src/tools/cargo). This function loads that file.
//
// The file is a newline-separated list of full commit hash, short commit hash, and commit date.
fn commit_info_from_rustc_source_tarball() -> Option<CommitInfo> {
    let path = Path::new("git-commit-info");
    if !path.exists() {
        return None;
    }

    // Dependency tracking is a nice to have for this (git doesn't do it), so if the path is not
    // valid UTF-8 just avoid doing it rather than erroring out.
    if let Some(utf8) = path.to_str() {
        output::rerun_if_changed(utf8);
    }

    let content = std::fs::read_to_string(&path).ok()?;
    let mut parts = content.split('\n').map(|s| s.to_string());
    Some(CommitInfo {
        hash: parts.next()?,
        short_hash: parts.next()?,
        date: parts.next()?,
    })
}

fn commit_info() {
    // Var set by bootstrap whenever omit-git-hash is enabled in rust-lang/rust's config.toml.
    output::rerun_if_env_changed("CFG_OMIT_GIT_HASH");
    #[expect(clippy::disallowed_methods, reason = "not a cargo buildscript input")]
    if std::env::var_os("CFG_OMIT_GIT_HASH").is_some() {
        return;
    }

    let Some(git) = commit_info_from_git().or_else(commit_info_from_rustc_source_tarball) else {
        return;
    };

    output::rustc_env("CARGO_COMMIT_HASH", &git.hash);
    output::rustc_env("CARGO_COMMIT_SHORT_HASH", &git.short_hash);
    output::rustc_env("CARGO_COMMIT_DATE", &git.date);
}

fn windows_manifest() {
    use std::env;
    let target_os = input::cargo_cfg_target_os();
    let target_env = input::cargo_cfg_target_env();
    if "windows" == &*target_os && Some("msvc") == target_env.as_deref() {
        static WINDOWS_MANIFEST_FILE: &str = "windows.manifest.xml";

        let mut manifest = env::current_dir().unwrap();
        manifest.push(WINDOWS_MANIFEST_FILE);

        output::rerun_if_changed(WINDOWS_MANIFEST_FILE);
        // Embed the Windows application manifest file.
        output::rustc_link_arg_bin("cargo", "/MANIFEST:EMBED");
        output::rustc_link_arg_bin(
            "cargo",
            &format!("/MANIFESTINPUT:{}", manifest.to_str().unwrap()),
        );
        // Turn linker warnings into errors.
        output::rustc_link_arg_bin("cargo", "/WX");
    }
}
