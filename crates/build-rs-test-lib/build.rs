fn main() {
    smoke_test_inputs();

    build_rs::output::rerun_if_changed("build.rs");
    build_rs::output::rustc_check_cfgs(&["did_run_build_script"]);
    build_rs::output::rustc_cfg("did_run_build_script");
}

fn smoke_test_inputs() {
    use build_rs::input::*;
    dbg!(cargo());
    dbg!(cargo_cfg_feature());
    dbg!(cargo_cfg("careful"));
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_fmt_debug());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_overflow_checks());
    dbg!(cargo_cfg_panic());
    dbg!(cargo_cfg_proc_macro());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_relocation_model());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_sanitize());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_sanitizer_cfi_generalize_pointers());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_sanitizer_cfi_normalize_integers());
    dbg!(cargo_cfg_target_abi());
    dbg!(cargo_cfg_target_arch());
    dbg!(cargo_cfg_target_endian());
    dbg!(cargo_cfg_target_env());
    dbg!(cargo_cfg_target_feature());
    dbg!(cargo_cfg_target_has_atomic());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_target_has_atomic_equal_alignment());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_target_has_atomic_load_store());
    dbg!(cargo_cfg_target_os());
    dbg!(cargo_cfg_target_pointer_width());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_target_thread_local());
    dbg!(cargo_cfg_target_vendor());
    #[cfg(feature = "unstable")]
    dbg!(cargo_cfg_ub_checks());
    dbg!(cargo_cfg_unix());
    dbg!(cargo_cfg_windows());
    dbg!(cargo_encoded_rustflags());
    dbg!(cargo_feature("unstable"));
    dbg!(cargo_manifest_dir());
    dbg!(cargo_manifest_path());
    dbg!(cargo_manifest_links());
    dbg!(cargo_pkg_authors());
    dbg!(cargo_pkg_description());
    dbg!(cargo_pkg_homepage());
    dbg!(cargo_pkg_license());
    dbg!(cargo_pkg_license_file());
    dbg!(cargo_pkg_name());
    dbg!(cargo_pkg_readme());
    dbg!(cargo_pkg_repository());
    dbg!(cargo_pkg_rust_version());
    dbg!(cargo_pkg_version());
    dbg!(cargo_pkg_version_major());
    dbg!(cargo_pkg_version_minor());
    dbg!(cargo_pkg_version_patch());
    dbg!(cargo_pkg_version_pre());
    dbg!(debug());
    dbg!(dep_metadata("z", "include"));
    dbg!(host());
    dbg!(num_jobs());
    dbg!(opt_level());
    dbg!(out_dir());
    dbg!(profile());
    dbg!(rustc());
    dbg!(rustc_linker());
    dbg!(rustc_workspace_wrapper());
    dbg!(rustc_wrapper());
    dbg!(rustdoc());
    dbg!(target());
}
