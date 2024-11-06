use std::sync::OnceLock;

fn rust_version_minor() -> u32 {
    static VERSION_MINOR: OnceLock<u32> = OnceLock::new();
    *VERSION_MINOR.get_or_init(|| {
        crate::input::cargo_pkg_rust_version()
            .split('.')
            .nth(1)
            .unwrap_or("70")
            .parse()
            .unwrap()
    })
}

pub(crate) fn double_colon_directives() -> bool {
    rust_version_minor() >= 77
}
