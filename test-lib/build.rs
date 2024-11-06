fn main() {
    build_rs::output::rerun_if_changed("build.rs");
    build_rs::output::rustc_cfg("did_run_build_script");
}
