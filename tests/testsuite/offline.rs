//! Tests for --offline flag.

use cargo_test_support::{
    basic_manifest, git, main_file, path2url, project,
    registry::{Package, RegistryBuilder},
    Execs,
};
use std::fs;

#[cargo_test]
fn offline_unused_target_dep() {
    // --offline with a target dependency that is not used and not downloaded.
    Package::new("unused_dep", "1.0.0").publish();
    Package::new("used_dep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [dependencies]
            used_dep = "1.0"
            [target.'cfg(unused)'.dependencies]
            unused_dep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    // Do a build that downloads only what is necessary.
    p.cargo("check")
        .with_stderr_contains("[DOWNLOADED] used_dep [..]")
        .with_stderr_does_not_contain("[DOWNLOADED] unused_dep [..]")
        .run();
    p.cargo("clean").run();
    // Build offline, make sure it works.
    p.cargo("check --offline").run();
}

#[cargo_test]
fn offline_missing_optional() {
    Package::new("opt_dep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [dependencies]
            opt_dep = { version = "1.0", optional = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    // Do a build that downloads only what is necessary.
    p.cargo("check")
        .with_stderr_does_not_contain("[DOWNLOADED] opt_dep [..]")
        .run();
    p.cargo("clean").run();
    // Build offline, make sure it works.
    p.cargo("check --offline").run();
    p.cargo("check --offline --features=opt_dep")
        .with_stderr(
            "\
[ERROR] failed to download `opt_dep v1.0.0`

Caused by:
  attempting to make an HTTP request, but --offline was specified
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_compile_path_with_offline() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check --offline").run();
}

#[cargo_test]
fn cargo_compile_with_downloaded_dependency_with_offline() {
    Package::new("present_dep", "1.2.3")
        .file("Cargo.toml", &basic_manifest("present_dep", "1.2.3"))
        .file("src/lib.rs", "")
        .publish();

    // make package downloaded
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2.3"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check").run();

    let p2 = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2.3"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p2.cargo("check --offline")
        .with_stderr(
            "\
[CHECKING] present_dep v1.2.3
[CHECKING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_offline_not_try_update() {
    // When --offline needs to download the registry, provide a reasonable
    // error hint to run without --offline.
    let p = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"

            [dependencies]
            not_cached_dep = "1.2.5"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let msg = "\
[ERROR] no matching package named `not_cached_dep` found
location searched: registry `crates-io`
required by package `bar v0.1.0 ([..]/bar)`
As a reminder, you're using offline mode (--offline) which can sometimes cause \
surprising resolution failures, if this error is too confusing you may wish to \
retry without the offline flag.
";

    p.cargo("check --offline")
        .with_status(101)
        .with_stderr(msg)
        .run();

    // While we're here, also check the config works.
    p.change_file(".cargo/config", "net.offline = true");
    p.cargo("check").with_status(101).with_stderr(msg).run();
}

#[cargo_test]
fn compile_offline_without_maxvers_cached() {
    Package::new("present_dep", "1.2.1").publish();
    Package::new("present_dep", "1.2.2").publish();

    Package::new("present_dep", "1.2.3")
        .file("Cargo.toml", &basic_manifest("present_dep", "1.2.3"))
        .file(
            "src/lib.rs",
            r#"pub fn get_version()->&'static str {"1.2.3"}"#,
        )
        .publish();

    Package::new("present_dep", "1.2.5")
        .file("Cargo.toml", &basic_manifest("present_dep", "1.2.5"))
        .file("src/lib.rs", r#"pub fn get_version(){"1.2.5"}"#)
        .publish();

    // make package cached
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "=1.2.3"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build").run();

    let p2 = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2"
            "#,
        )
        .file(
            "src/main.rs",
            "\
extern crate present_dep;
fn main(){
    println!(\"{}\", present_dep::get_version());
}",
        )
        .build();

    p2.cargo("run --offline")
        .with_stderr(
            "\
[COMPILING] present_dep v1.2.3
[COMPILING] foo v0.1.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
     Running `[..]`",
        )
        .with_stdout("1.2.3")
        .run();
}

#[cargo_test]
fn cargo_compile_forbird_git_httpsrepo_offline() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"

            [package]
            name = "foo"
            version = "0.5.0"
            authors = ["chabapok@example.com"]

            [dependencies.dep1]
            git = 'https://github.com/some_user/dep1.git'
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check --offline").with_status(101).with_stderr("\
[ERROR] failed to get `dep1` as a dependency of package `foo v0.5.0 [..]`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  Unable to update https://github.com/some_user/dep1.git

Caused by:
  can't checkout from 'https://github.com/some_user/dep1.git': you are in the offline mode (--offline)").run();
}

#[cargo_test]
fn compile_offline_while_transitive_dep_not_cached() {
    let baz = Package::new("baz", "1.0.0");
    let baz_path = baz.archive_dst();
    baz.publish();

    let baz_content = fs::read(&baz_path).unwrap();
    // Truncate the file to simulate a download failure.
    fs::write(&baz_path, &[]).unwrap();

    Package::new("bar", "0.1.0").dep("baz", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    // simulate download bar, but fail to download baz
    p.cargo("check")
        .with_status(101)
        .with_stderr_contains("[..]failed to verify the checksum of `baz[..]")
        .run();

    // Restore the file contents.
    fs::write(&baz_path, &baz_content).unwrap();

    p.cargo("check --offline")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to download `bar v0.1.0`

Caused by:
  attempting to make an HTTP request, but --offline was specified
",
        )
        .run();
}

fn update_offline_not_cached() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("update --offline")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] no matching package named `bar` found
location searched: registry `[..]`
required by package `foo v0.0.1 ([..]/foo)`
As a reminder, you're using offline mode (--offline) which can sometimes cause \
surprising resolution failures, if this error is too confusing you may wish to \
retry without the offline flag.",
        )
        .run();
}

#[cargo_test]
fn update_offline_not_cached_sparse() {
    let _registry = RegistryBuilder::new().http_index().build();
    update_offline_not_cached()
}

#[cargo_test]
fn update_offline_not_cached_git() {
    update_offline_not_cached()
}

#[cargo_test]
fn cargo_compile_offline_with_cached_git_dep() {
    compile_offline_with_cached_git_dep(false)
}

#[cargo_test]
fn gitoxide_cargo_compile_offline_with_cached_git_dep_shallow_dep() {
    compile_offline_with_cached_git_dep(true)
}

fn compile_offline_with_cached_git_dep(shallow: bool) {
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
            .file(
                "src/lib.rs",
                r#"
                pub static COOL_STR:&str = "cached git repo rev1";
                "#,
            )
    });

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Commit the changes and make sure we trigger a recompile
    git_project.change_file(
        "src/lib.rs",
        r#"pub static COOL_STR:&str = "cached git repo rev2";"#,
    );
    git::add(&repo);
    let rev2 = git::commit(&repo);

    // cache to registry rev1 and rev2
    let prj = project()
        .at("cache_git_dep")
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "cache_git_dep"
                version = "0.5.0"

                [dependencies.dep1]
                git = '{}'
                rev = "{}"
                "#,
                git_project.url(),
                rev1
            ),
        )
        .file("src/main.rs", "fn main(){}")
        .build();
    let maybe_use_shallow = |mut cargo: Execs| -> Execs {
        if shallow {
            cargo
                .arg("-Zgitoxide=fetch,shallow-deps")
                .masquerade_as_nightly_cargo(&[
                    "unstable features must be available for -Z gitoxide",
                ]);
        }
        cargo
    };
    maybe_use_shallow(prj.cargo("build")).run();

    prj.change_file(
        "Cargo.toml",
        &format!(
            r#"
            [package]
            name = "cache_git_dep"
            version = "0.5.0"

            [dependencies.dep1]
            git = '{}'
            rev = "{}"
            "#,
            git_project.url(),
            rev2
        ),
    );
    maybe_use_shallow(prj.cargo("build")).run();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.5.0"

                [dependencies.dep1]
                git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            &main_file(r#""hello from {}", dep1::COOL_STR"#, &["dep1"]),
        )
        .build();

    let git_root = git_project.root();

    let mut cargo = p.cargo("build --offline");
    cargo.with_stderr(format!(
        "\
[COMPILING] dep1 v0.5.0 ({}#[..])
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        path2url(git_root),
    ));
    maybe_use_shallow(cargo).run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout("hello from cached git repo rev2\n")
        .run();

    p.change_file(
        "Cargo.toml",
        &format!(
            r#"
            [package]
            name = "foo"
            version = "0.5.0"

            [dependencies.dep1]
            git = '{}'
            rev = "{}"
            "#,
            git_project.url(),
            rev1
        ),
    );

    maybe_use_shallow(p.cargo("build --offline")).run();
    p.process(&p.bin("foo"))
        .with_stdout("hello from cached git repo rev1\n")
        .run();
}

#[cargo_test]
fn offline_resolve_optional_fail() {
    // Example where resolve fails offline.
    //
    // This happens if at least 1 version of an optional dependency is
    // available, but none of them satisfy the requirements. The current logic
    // that handles this is `RegistryIndex::query_inner`, and it doesn't know
    // if the package being queried is an optional one. This is not ideal, it
    // would be best if it just ignored optional (unselected) dependencies.
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = { version = "1.0", optional = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch").run();

    // Change dep to 2.0.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = { version = "2.0", optional = true }
        "#,
    );

    p.cargo("check --offline")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to select a version for the requirement `dep = \"^2.0\"`
candidate versions found which didn't match: 1.0.0
location searched: `[..]` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([..]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?
As a reminder, you're using offline mode (--offline) which can sometimes cause \
surprising resolution failures, if this error is too confusing you may wish to \
retry without the offline flag.
",
        )
        .run();
}

#[cargo_test]
fn offline_with_all_patched() {
    // Offline works if everything is patched.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = "1.0"

            [patch.crates-io]
            dep = {path = "dep"}
            "#,
        )
        .file("src/lib.rs", "pub fn f() { dep::foo(); }")
        .file("dep/Cargo.toml", &basic_manifest("dep", "1.0.0"))
        .file("dep/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("check --offline").run();
}

#[cargo_test]
fn update_offline_cached() {
    // Cache a few versions to update against
    let p = project().file("src/lib.rs", "").build();
    let versions = ["1.2.3", "1.2.5", "1.2.9"];
    for vers in versions.iter() {
        Package::new("present_dep", vers)
            .file("Cargo.toml", &basic_manifest("present_dep", vers))
            .file(
                "src/lib.rs",
                format!(r#"pub fn get_version()->&'static str {{ "{}" }}"#, vers).as_str(),
            )
            .publish();
        // make package cached
        p.change_file(
            "Cargo.toml",
            format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                present_dep = "={}"
                "#,
                vers
            )
            .as_str(),
        );
        p.cargo("build").run();
    }

    let p2 = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            present_dep = "1.2"
            "#,
        )
        .file(
            "src/main.rs",
            "\
extern crate present_dep;
fn main(){
    println!(\"{}\", present_dep::get_version());
}",
        )
        .build();

    p2.cargo("build --offline")
        .with_stderr(
            "\
[COMPILING] present_dep v1.2.9
[COMPILING] foo v0.1.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p2.rename_run("foo", "with_1_2_9")
        .with_stdout("1.2.9")
        .run();
    // updates happen without updating the index
    p2.cargo("update present_dep --precise 1.2.3 --offline")
        .with_status(0)
        .with_stderr(
            "\
[DOWNGRADING] present_dep v1.2.9 -> v1.2.3
",
        )
        .run();

    p2.cargo("build --offline")
        .with_stderr(
            "\
[COMPILING] present_dep v1.2.3
[COMPILING] foo v0.1.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p2.rename_run("foo", "with_1_2_3")
        .with_stdout("1.2.3")
        .run();

    // Offline update should only print package details and not index updating
    p2.cargo("update --offline")
        .with_status(0)
        .with_stderr(
            "\
[UPDATING] present_dep v1.2.3 -> v1.2.9
",
        )
        .run();

    // No v1.2.8 loaded into the cache so expect failure.
    p2.cargo("update present_dep --precise 1.2.8 --offline")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] no matching package named `present_dep` found
location searched: registry `[..]`
required by package `foo v0.1.0 ([..]/foo)`
As a reminder, you're using offline mode (--offline) which can sometimes cause \
surprising resolution failures, if this error is too confusing you may wish to \
retry without the offline flag.
",
        )
        .run();
}

#[cargo_test]
fn offline_and_frozen_and_no_lock() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check --frozen --offline")
        .with_status(101)
        .with_stderr("\
error: the lock file [ROOT]/foo/Cargo.lock needs to be updated but --frozen was passed to prevent this
If you want to try to generate the lock file without accessing the network, \
remove the --frozen flag and use --offline instead.
")
        .run();
}

#[cargo_test]
fn offline_and_locked_and_no_frozen() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check --locked --offline")
        .with_status(101)
        .with_stderr("\
error: the lock file [ROOT]/foo/Cargo.lock needs to be updated but --locked was passed to prevent this
If you want to try to generate the lock file without accessing the network, \
remove the --locked flag and use --offline instead.
")
        .run();
}
