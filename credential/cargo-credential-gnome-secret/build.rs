fn main() {
    if cfg!(target_os = "linux") {
        // TODO: Consider ignoring errors when libsecret is not installed and
        // switching the impl to UnsupportedCredential (possibly along with a
        // warning?).
        pkg_config::probe_library("libsecret-1").unwrap();
    }
}
