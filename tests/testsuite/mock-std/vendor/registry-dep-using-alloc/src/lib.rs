#[cfg(feature = "mockbuild")]
pub fn custom_api() {
}

#[cfg(not(feature = "mockbuild"))]
pub fn non_sysroot_api() {
    core::custom_api();
    alloc::custom_api();
}