use std::{process::Command, sync::OnceLock};

fn rust_version_minor() -> u32 {
    static VERSION_MINOR: OnceLock<u32> = OnceLock::new();
    *VERSION_MINOR.get_or_init(|| {
        version_minor(&crate::input::cargo_pkg_rust_version().unwrap_or_default())
            // assume build-rs's MSRV if none specified for the current package
            .unwrap_or_else(|| version_minor(env!("CARGO_PKG_RUST_VERSION")).unwrap())
    })
}

fn cargo_version_minor() -> u32 {
    static VERSION_MINOR: OnceLock<u32> = OnceLock::new();
    *VERSION_MINOR.get_or_init(|| {
        let out = Command::new(crate::input::cargo())
            .arg("-V")
            .output()
            .expect("running `cargo -V` should succeed");
        assert!(out.status.success(), "running `cargo -V` should succeed");

        // > cargo -V # example output
        // cargo 1.82.0 (8f40fc59f 2024-08-21)

        let out = std::str::from_utf8(&out.stdout).expect("`cargo -V` should output valid UTF-8");
        let version = out.split(' ').nth(1).unwrap();
        version_minor(version).unwrap()
    })
}

fn version_minor(version: &str) -> Option<u32> {
    let minor = version.split('.').nth(1)?;
    let minor = minor.parse().unwrap();
    Some(minor)
}

pub(crate) fn double_colon_directives() -> bool {
    // cargo errors on `cargo::` directives with insufficient package.rust-version
    rust_version_minor() >= 77
}

pub(crate) fn check_cfg() -> bool {
    // emit check-cfg if the toolchain being used supports it
    cargo_version_minor() >= 80
}
