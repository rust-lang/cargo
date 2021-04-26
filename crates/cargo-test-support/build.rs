fn main() {
    println!(
        "cargo:rustc-env=NATIVE_ARCH={}",
        std::env::var("TARGET").unwrap()
    );
}
