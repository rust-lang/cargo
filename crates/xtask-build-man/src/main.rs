//! ```text
//! NAME
//!         build-man
//!
//! SYNOPSIS
//!         build-man
//!
//! DESCRIPTION
//!         Build the man pages for packages `mdman` and `cargo`.
//!         For more, read their doc comments.
//! ```

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;
use std::process::Command;

fn main() -> io::Result<()> {
    build_mdman()?;
    build_cargo()?;
    Ok(())
}

/// Builds the man pages for `mdman`.
fn build_mdman() -> io::Result<()> {
    cwd_to_workspace_root()?;

    let src_paths = &["crates/mdman/doc/mdman.md".into()];
    let dst_path = "crates/mdman/doc/out";
    let outs = [("md", dst_path), ("txt", dst_path), ("man", dst_path)];

    build_man("mdman", src_paths, &outs, &[])
}

/// Builds the man pages for Cargo.
///
/// The source for the man pages are located in src/doc/man/ in markdown format.
/// These also are handlebars templates, see crates/mdman/README.md for details.
///
/// The generated man pages are placed in the src/etc/man/ directory. The pages
/// are also expanded into markdown (after being expanded by handlebars) and
/// saved in the src/doc/src/commands/ directory. These are included in the
/// Cargo book, which is converted to HTML by mdbook.
fn build_cargo() -> io::Result<()> {
    // Find all `src/doc/man/cargo-*.md`
    let src_paths = {
        let mut src_paths = Vec::new();
        for entry in fs::read_dir("src/doc/man")? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_str().unwrap();
            if file_name.starts_with("cargo-") && file_name.ends_with(".md") {
                src_paths.push(entry.path());
            }
        }
        src_paths
    };
    let outs = [
        ("md", "src/doc/src/commands"),
        ("txt", "src/doc/man/generated_txt"),
        ("man", "src/etc/man"),
    ];
    let args = [
        "--url",
        "https://doc.rust-lang.org/cargo/commands/",
        "--man",
        "rustc:1=https://doc.rust-lang.org/rustc/index.html",
        "--man",
        "rustdoc:1=https://doc.rust-lang.org/rustdoc/index.html",
    ];
    build_man("cargo", &src_paths[..], &outs, &args)
}

/// Change to workspace root.
///
/// Assumed this xtask is located in `[WORKSPACE]/crates/xtask-build-man`.
fn cwd_to_workspace_root() -> io::Result<()> {
    let pkg_root = std::env!("CARGO_MANIFEST_DIR");
    let ws_root = format!("{pkg_root}/../..");
    std::env::set_current_dir(ws_root)
}

/// Builds the man pages.
fn build_man(
    pkg_name: &str,
    src_paths: &[PathBuf],
    outs: &[(&str, &str)],
    extra_args: &[&str],
) -> io::Result<()> {
    for (format, dst_path) in outs {
        eprintln!("Start converting `{format}` for package `{pkg_name}`...");
        let mut cmd = Command::new(std::env!("CARGO"));
        cmd.args(["run", "--package", "mdman", "--"])
            .args(["-t", format, "-o", dst_path])
            .args(src_paths)
            .args(extra_args);

        let status = cmd.status()?;
        if !status.success() {
            eprintln!("failed to build the man pages for package `{pkg_name}`");
            eprintln!("failed command: `{cmd:?}`");
            process::exit(status.code().unwrap_or(1));
        }
    }

    Ok(())
}
