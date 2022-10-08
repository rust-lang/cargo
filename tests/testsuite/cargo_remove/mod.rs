mod avoid_empty_tables;
mod build;
mod dev;
mod dry_run;
mod gc_patch;
mod gc_profile;
mod gc_replace;
mod invalid_arg;
mod invalid_dep;
mod invalid_package;
mod invalid_package_multiple;
mod invalid_section;
mod invalid_section_dep;
mod invalid_target;
mod invalid_target_dep;
mod multiple_deps;
mod multiple_dev;
mod no_arg;
mod offline;
mod optional_dep_feature;
mod optional_feature;
mod package;
mod remove_basic;
mod target;
mod target_build;
mod target_dev;
mod update_lock_file;
mod workspace;
mod workspace_non_virtual;
mod workspace_preserved;

fn init_registry() {
    cargo_test_support::registry::init();
    add_registry_packages(false);
}

fn add_registry_packages(alt: bool) {
    for name in [
        "clippy",
        "dbus",
        "docopt",
        "ncurses",
        "pad",
        "regex",
        "rustc-serialize",
        "toml",
    ] {
        cargo_test_support::registry::Package::new(name, "0.1.1+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.2.0+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.2.3+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.4.1+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.6.2+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.9.9+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "1.0.90+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "20.0.0+my-package")
            .alternative(alt)
            .publish();
    }

    for name in ["semver", "serde"] {
        cargo_test_support::registry::Package::new(name, "0.1.1")
            .alternative(alt)
            .feature("std", &[])
            .publish();
        cargo_test_support::registry::Package::new(name, "0.9.0")
            .alternative(alt)
            .feature("std", &[])
            .publish();
        cargo_test_support::registry::Package::new(name, "1.0.90")
            .alternative(alt)
            .feature("std", &[])
            .publish();
    }
}
