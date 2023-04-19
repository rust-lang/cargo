fn main() {
    pkg_config::probe_library("libsecret-1").unwrap();
}
