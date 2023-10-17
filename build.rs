use flate2::{Compression, GzBuilder};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    commit_info();
    compress_man();
    // ALLOWED: Accessing environment during build time shouldn't be prohibited.
    #[allow(clippy::disallowed_methods)]
    let target = std::env::var("TARGET").unwrap();
    println!("cargo:rustc-env=RUST_HOST_TARGET={target}");
}

fn compress_man() {
    // ALLOWED: Accessing environment during build time shouldn't be prohibited.
    #[allow(clippy::disallowed_methods)]
    let out_path = Path::new(&std::env::var("OUT_DIR").unwrap()).join("man.tgz");
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
            println!("cargo:rerun-if-changed={}", path.display());
            ar.append_path_with_name(&path, path.file_name().unwrap())
                .unwrap();
        }
    };

    add_files(Path::new("src/etc/man"), OsStr::new("1"));
    add_files(Path::new("src/doc/man/generated_txt"), OsStr::new("txt"));
    let encoder = ar.into_inner().unwrap();
    encoder.finish().unwrap();
}

fn commit_info() {
    if !Path::new(".git").exists() {
        return;
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
        _ => return,
    };
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut parts = stdout.split_whitespace();
    let mut next = || parts.next().unwrap();
    println!("cargo:rustc-env=CARGO_COMMIT_HASH={}", next());
    println!("cargo:rustc-env=CARGO_COMMIT_SHORT_HASH={}", next());
    println!("cargo:rustc-env=CARGO_COMMIT_DATE={}", next())
}
