fn main() {
    println!(
        "cargo:rustc-env=NATIVE_ARCH={}",
        std::env::var("TARGET").unwrap()
    );
    println!("cargo:rerun-if-changed=build.rs");
}
