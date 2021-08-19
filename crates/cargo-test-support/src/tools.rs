//! Common executables that can be reused by various tests.

use crate::{basic_manifest, paths, project, Project};
use lazy_static::lazy_static;
use std::path::PathBuf;
use std::sync::Mutex;

lazy_static! {
    static ref ECHO_WRAPPER: Mutex<Option<PathBuf>> = Mutex::new(None);
}

/// Returns the path to an executable that works as a wrapper around rustc.
///
/// The wrapper will echo the command line it was called with to stderr.
pub fn echo_wrapper() -> PathBuf {
    let mut lock = ECHO_WRAPPER.lock().unwrap();
    if let Some(path) = &*lock {
        return path.clone();
    }
    let p = project()
        .at(paths::global_root().join("rustc-echo-wrapper"))
        .file("Cargo.toml", &basic_manifest("rustc-echo-wrapper", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                let args = std::env::args().collect::<Vec<_>>();
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
