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

#![allow(clippy::print_stderr)]

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::process::Command;

use flate2::Compression;
use flate2::GzBuilder;

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
/// The source for the man pages are located in doc/man/ in markdown format.
/// These also are handlebars templates, see crates/mdman/README.md for details.
///
/// The generated man pages are placed in the etc/man/ directory. The pages
/// are also expanded into markdown (after being expanded by handlebars) and
/// saved in the doc/book/src/commands/ directory. These are included in the
/// Cargo book, which is converted to HTML by mdbook.
///
/// The troff and plain text files are also bundled in `doc/man/man.tgz`,
/// which is embedded in the main `cargo` executable.
fn build_cargo() -> io::Result<()> {
    // Find all `doc/man/cargo*.md`
    let src_paths = {
        let mut src_paths = Vec::new();
        for entry in fs::read_dir("doc/man")? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_str().unwrap();
            if file_name.starts_with("cargo") && file_name.ends_with(".md") {
                src_paths.push(entry.path());
            }
        }
        src_paths
    };
    let outs = [
        ("md", "doc/book/src/commands"),
        ("txt", "doc/man/generated_txt"),
        ("man", "etc/man"),
    ];
    let args = [
        "--url",
        "https://doc.rust-lang.org/cargo/commands/",
        "--man",
        "rustc:1=https://doc.rust-lang.org/rustc/index.html",
        "--man",
        "rustdoc:1=https://doc.rust-lang.org/rustdoc/index.html",
    ];
    build_man("cargo", &src_paths[..], &outs, &args)?;
    compress_man();
    Ok(())
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

fn compress_man() {
    let out_path = Path::new("doc/man/man.tgz");
    let dst = fs::File::create(out_path).unwrap();
    let encoder = GzBuilder::new()
        .filename("man.tar")
        .write(dst, Compression::best());
    let mut ar = tar::Builder::new(encoder);
    ar.mode(tar::HeaderMode::Deterministic);

    let mut add_files = |dir, extension| {
        let mut files = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            if path.extension() != Some(extension) {
                continue;
            }
            ar.append_path_with_name(&path, path.file_name().unwrap())
                .unwrap();
        }
    };

    add_files(Path::new("etc/man"), OsStr::new("1"));
    add_files(Path::new("doc/man/generated_txt"), OsStr::new("txt"));
    let encoder = ar.into_inner().unwrap();
    encoder.finish().unwrap();
}
