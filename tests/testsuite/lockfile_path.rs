//! Tests for `lockfile-path` flag

use std::fs;

use snapbox::str;

use cargo_test_support::registry::RegistryBuilder;
use cargo_test_support::{
    basic_bin_manifest, cargo_test, project, symlink_supported, Execs,
    ProjectBuilder,
};

const VALID_LOCKFILE: &str = r#"# Test lockfile
version = 3

[[package]]
name = "test_foo"
version = "0.5.0"
"#;

const LIB_TOML: &str = r#"
        [package]
        name = "test_bar"
        version = "0.1.0"
        edition = "2021"
    "#;

fn make_basic_project() -> ProjectBuilder {
    return project()
        .file("Cargo.toml", &basic_bin_manifest("test_foo"))
        .file("src/main.rs", "fn main() {}");
}

fn make_basic_command(execs: &mut Execs, lockfile_path_argument: String) -> &mut Execs {
    return execs
        .masquerade_as_nightly_cargo(&["unstable-options"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path_argument);
}

fn lockfile_must_exist(command: &str) -> bool {
    return command == "pkgid" || command == "publish" || command == "package";
}

fn assert_lockfile_created(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    if lockfile_must_exist(command) {
        return;
    }

    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project().build();
    let registry = RegistryBuilder::new().http_api().http_index().build();

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .replace_crates_io(registry.index_url())
        .run();
    assert!(!p.root().join("Cargo.lock").is_file());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn assert_lockfile_read(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();
    let registry = RegistryBuilder::new().http_api().http_index().build();

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .replace_crates_io(registry.index_url())
        .run();

    assert!(!p.root().join("Cargo.lock").is_file());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn assert_lockfile_override(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    if lockfile_must_exist(command) {
        return;
    }

    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    let registry = RegistryBuilder::new().http_api().http_index().build();

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .replace_crates_io(registry.index_url())
        .run();

    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn assert_symlink_in_path(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    if !symlink_supported() || lockfile_must_exist(command) {
        return;
    }

    let dst = "dst";
    let src = "somedir/link";
    let lockfile_path_argument = format!("{src}/Cargo.lock");

    let p = make_project().symlink_dir(dst, src).build();
    let registry = RegistryBuilder::new().http_api().http_index().build();

    fs::create_dir(p.root().join("dst"))
        .unwrap_or_else(|e| panic!("could not create directory {}", e));
    assert!(p.root().join(src).is_dir());

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .replace_crates_io(registry.index_url())
        .run();

    assert!(p.root().join(format!("{src}/Cargo.lock")).is_file());
    assert!(p.root().join(lockfile_path_argument).is_file());
    assert!(p.root().join(dst).join("Cargo.lock").is_file());
}

fn assert_symlink_lockfile(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    if !symlink_supported() {
        return;
    }

    let lockfile_path_argument = "dst/Cargo.lock";
    let src = "somedir/link";
    let lock_body = VALID_LOCKFILE;

    let p = make_project()
        .file(lockfile_path_argument, lock_body)
        .symlink(lockfile_path_argument, src)
        .build();
    let registry = RegistryBuilder::new().http_api().http_index().build();

    assert!(p.root().join(src).is_file());

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .replace_crates_io(registry.index_url())
        .run();

    assert!(!p.root().join("Cargo.lock").is_file());
}

fn assert_broken_symlink(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    if !symlink_supported() {
        return;
    }

    let invalid_dst = "invalid_path";
    let src = "somedir/link";
    let lockfile_path_argument = format!("{src}/Cargo.lock");

    let p = make_project().symlink_dir(invalid_dst, src).build();
    assert!(!p.root().join(src).is_dir());
    let registry = RegistryBuilder::new().http_api().http_index().build();

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] Failed to create lockfile-path parent directory somedir/link

Caused by:
  File exists (os error 17)

"#]])
        .replace_crates_io(registry.index_url())
        .run();
}

fn assert_loop_symlink(
    command: &str,
    make_execs: impl Fn(&mut Execs, String) -> &mut Execs,
    make_project: impl FnOnce() -> ProjectBuilder,
) {
    if !symlink_supported() {
        return;
    }

    let loop_link = "loop";
    let src = "somedir/link";
    let lockfile_path_argument = format!("{src}/Cargo.lock");

    let p = make_project()
        .symlink_dir(loop_link, src)
        .symlink_dir(src, loop_link)
        .build();
    assert!(!p.root().join(src).is_dir());
    let registry = RegistryBuilder::new().http_api().http_index().build();

    make_execs(&mut p.cargo(command), lockfile_path_argument.to_string())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] Failed to fetch lock file's parent path metadata somedir/link

Caused by:
  Too many levels of symbolic links (os error 40)

"#]])
        .replace_crates_io(registry.index_url())
        .run();
}

/////////////////////
//// Generic tests
/////////////////////

macro_rules! tests {
    ($name: ident, $cmd_name:expr, $make_command:expr, $setup_test:expr) => {
        #[cfg(test)]
        mod $name {
            use super::*;

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_lockfile_created() {
                assert_lockfile_created($cmd_name, $make_command, $setup_test);
            }

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_lockfile_read() {
                assert_lockfile_read($cmd_name, $make_command, $setup_test);
            }

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_lockfile_override() {
                assert_lockfile_override($cmd_name, $make_command, $setup_test);
            }

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_symlink_in_path() {
                assert_symlink_in_path($cmd_name, $make_command, $setup_test);
            }

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_symlink_lockfile() {
                assert_symlink_lockfile($cmd_name, $make_command, $setup_test);
            }

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_broken_symlink() {
                assert_broken_symlink($cmd_name, $make_command, $setup_test);
            }

            #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
            fn test_loop_symlink() {
                assert_loop_symlink($cmd_name, $make_command, $setup_test);
            }
        }
    };

    ($name: ident, $cmd_name:expr) => {
        tests!($name, $cmd_name, make_basic_command, make_basic_project);
    };
}

fn make_add_command(execs: &mut Execs, lockfile_path_argument: String) -> &mut Execs {
    return make_basic_command(execs, lockfile_path_argument)
        .arg("--path")
        .arg("../bar");
}

fn make_add_project() -> ProjectBuilder {
    return make_basic_project()
        .file("../bar/Cargo.toml", LIB_TOML)
        .file("../bar/src/main.rs", "fn main() {}");
}

fn make_clean_command(execs: &mut Execs, lockfile_path_argument: String) -> &mut Execs {
    return make_basic_command(execs, lockfile_path_argument)
        .arg("--package")
        .arg("test_foo");
}

fn make_fix_command(execs: &mut Execs, lockfile_path_argument: String) -> &mut Execs {
    return make_basic_command(execs, lockfile_path_argument)
        .arg("--package")
        .arg("test_foo")
        .arg("--allow-no-vcs");
}

fn make_remove_project() -> ProjectBuilder {
    let mut manifest = basic_bin_manifest("test_foo");
    manifest.push_str(
        r#"#
[dependencies]
test_bar = { version = "0.1.0", path = "../bar" }
"#,
    );

    return project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", "fn main() {}")
        .file("../bar/Cargo.toml", LIB_TOML)
        .file("../bar/src/main.rs", "fn main() {}");
}

fn make_remove_command(execs: &mut Execs, lockfile_path_argument: String) -> &mut Execs {
    return make_basic_command(execs, lockfile_path_argument).arg("test_bar");
}

tests!(add, "add", make_add_command, make_add_project);
tests!(bench, "bench");
tests!(build, "build");
tests!(check, "check");
tests!(clean, "clean", make_clean_command, make_basic_project);
tests!(doc, "doc");
tests!(fetch, "fetch");
tests!(fix, "fix", make_fix_command, make_basic_project);
tests!(generate_lockfile, "generate-lockfile");
tests!(metadata, "metadata");
tests!(package, "package");
tests!(pkgid, "pkgid");
tests!(publish, "publish");
tests!(remove, "remove", make_remove_command, make_remove_project);
tests!(run, "run");
tests!(rustc, "rustc");
tests!(rustdoc, "rustdoc");
tests!(test, "test");
tests!(tree, "tree");
tests!(update, "update");
tests!(vendor, "vendor");

#[cfg(test)]
mod package_extra {
    use std::fs;
    use cargo_test_support::{cargo_test, project};
    use crate::lockfile_path::{make_basic_command};

    #[cargo_test(nightly, reason = "--lockfile-path is unstable")]
    fn assert_respect_pinned_version_from_lockfile_path() {
        const PINNED_VERSION: &str = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "hello"
version = "1.0.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "2b94f16c310ebbd9efcca5a5a17131d70bd454876f2af007f3da211afadff4fc"

[[package]]
name = "test_foo"
version = "0.5.0"
dependencies = [
 "hello",
]
"#;
        const TOML: &str = r#"#
[package]

name = "test_foo"
version = "0.5.0"
authors = ["wycats@example.com"]
edition = "2015"

[[bin]]

name = "test_foo"

[dependencies]
hello = "1.0.0"
"#;
        
        let lockfile_path_argument = "mylockfile/Cargo.lock";
        let p = project()
            .file("Cargo.toml", TOML)
            .file("src/main.rs", "fn main() {}")
            .file("mylockfile/Cargo.lock", PINNED_VERSION)
            .build();
        
        make_basic_command(&mut p.cargo("package"), lockfile_path_argument.to_string()).run();

        assert!(!p.root().join("Cargo.lock").is_file());
        assert!(p.root().join(lockfile_path_argument).is_file());
        
        assert!(p.root().join("target/package/test_foo-0.5.0/Cargo.lock").is_file());
        
        let path = p.root().join("target/package/test_foo-0.5.0/Cargo.lock");
        let contents = fs::read_to_string(path).unwrap();
        
        assert_eq!(contents, PINNED_VERSION);
    }
}
