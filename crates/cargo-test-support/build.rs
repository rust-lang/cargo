#![allow(clippy::disallowed_methods)]

fn main() {
    println!("cargo:rustc-check-cfg=cfg(emulate_second_only_system)");
    println!(
        "cargo:rustc-env=NATIVE_ARCH={}",
        std::env::var("TARGET").unwrap()
    );
    println!("cargo:rerun-if-changed=build.rs");
}
