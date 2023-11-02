//! Common executables that can be reused by various tests.

use crate::{basic_manifest, paths, project, Project};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;

static ECHO_WRAPPER: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static ECHO: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Returns the path to an executable that works as a wrapper around rustc.
///
/// The wrapper will echo the command line it was called with to stderr.
pub fn echo_wrapper() -> PathBuf {
    let mut lock = ECHO_WRAPPER
        .get_or_init(|| Default::default())
        .lock()
        .unwrap();
    if let Some(path) = &*lock {
        return path.clone();
    }
    let p = project()
        .at(paths::global_root().join("rustc-echo-wrapper"))
        .file("Cargo.toml", &basic_manifest("rustc-echo-wrapper", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
            use std::fs::read_to_string;
            use std::path::PathBuf;
            fn main() {
                // Handle args from `@path` argfile for rustc
                let args = std::env::args()
                    .flat_map(|p| if let Some(p) = p.strip_prefix("@") {
                        read_to_string(p).unwrap().lines().map(String::from).collect()
                    } else {
                        vec![p]
                    })
                    .collect::<Vec<_>>();
                eprintln!("WRAPPER CALLED: {}", args[1..].join(" "));
                let status = std::process::Command::new(&args[1])
                    .args(&args[2..]).status().unwrap();
                std::process::exit(status.code().unwrap_or(1));
            }
            "#,
        )
        .build();
    p.cargo("build").run();
    let path = p.bin("rustc-echo-wrapper");
    *lock = Some(path.clone());
    path
}

/// Returns the path to an executable that prints its arguments.
///
/// Do not expect this to be anything fancy.
pub fn echo() -> PathBuf {
    let mut lock = ECHO.get_or_init(|| Default::default()).lock().unwrap();
    if let Some(path) = &*lock {
        return path.clone();
    }
    if let Ok(path) = cargo_util::paths::resolve_executable(Path::new("echo")) {
        *lock = Some(path.clone());
        return path;
    }
    // Often on Windows, `echo` is not available.
    let p = project()
        .at(paths::global_root().join("basic-echo"))
        .file("Cargo.toml", &basic_manifest("basic-echo", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let mut s = String::new();
                    let mut it = std::env::args().skip(1).peekable();
                    while let Some(n) = it.next() {
                        s.push_str(&n);
                        if it.peek().is_some() {
                            s.push(' ');
                        }
                    }
                    println!("{}", s);
                }
            "#,
        )
        .build();
    p.cargo("build").run();
    let path = p.bin("basic-echo");
    *lock = Some(path.clone());
    path
}

/// Returns a project which builds a cargo-echo simple subcommand
pub fn echo_subcommand() -> Project {
    let p = project()
        .at("cargo-echo")
        .file("Cargo.toml", &basic_manifest("cargo-echo", "0.0.1"))
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let args: Vec<_> = ::std::env::args().skip(1).collect();
                    println!("{}", args.join(" "));
                }
            "#,
        )
        .build();
    p.cargo("build").run();
    p
}
