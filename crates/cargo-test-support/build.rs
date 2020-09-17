use std::env;
use std::fs;
use std::path::Path;

static CARGO_INTEGRATION_TEST_DIR: &str = "cit";

fn main() {
    let target_dir = env::var_os("CARGO_TARGET_DIR").unwrap();
    let test_dir = Path::new(&target_dir).join(CARGO_INTEGRATION_TEST_DIR);
    if let Err(e) = fs::create_dir_all(&test_dir) {
        panic!(
            "failed to create directory for integration tests ({}): {}",
            test_dir.display(),
            e,
        );
    }
    println!("cargo:rustc-env=GLOBAL_ROOT={}", test_dir.display());
}
