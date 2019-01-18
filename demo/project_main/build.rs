use std::process::Command;
use std::path;

fn main() {
    let workspace = path::Path::new("../").canonicalize().unwrap();
    eprintln!("building via cargo-web");
    let status = Command::new("cargo")
        .current_dir(workspace)
        .args(&[
            "build",
            "-p",
            "project_wasm",
            "--target=wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .unwrap();

    assert!(status.success(), "project_wasm failed to build");

}
