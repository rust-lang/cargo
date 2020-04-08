//! Tests of cargo build --dependencies and --remote-dependencies

use cargo_test_support::{basic_lib_manifest, git, main_file, project, registry};

#[cargo_test]
fn build_deps_basic() {
    // Setup a project with two local (by path) dependencies
    // foo -> bar -> baz
    //
    // Test that with --dependencies only bar & baz are built & cached

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "../bar" }
        "#,
        )
        .file("src/main.rs", &main_file(r#""I am foo""#, &["bar"]))
        .build();

    project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            baz = { path = "../baz" }
        "#,
        )
        .file("src/lib.rs", &main_file(r#""I am bar""#, &["baz"]))
        .build();

    project()
        .at("baz")
        .file("Cargo.toml", &basic_lib_manifest("baz"))
        .file("src/lib.rs", &main_file(r#""I am baz""#, &[]))
        .build();

    p.cargo("build -Z unstable-options --dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();

    // The `bar` & `baz` dependencies should now be built
    // but the top-level `foo` bin should *not* be built
    assert!(p.dep_built("baz"));
    assert!(p.dep_built("bar"));
    assert!(!p.bin("foo").is_file());

    // A subsequent build command should build `foo` only
    p.cargo("build")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();

    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn build_deps_remote() {
    // Setup a project with both local and remote dependencies
    // foo -> bar (path) -> baz (git)

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = "../bar" }
        "#,
        )
        .file("src/main.rs", &main_file(r#""I am foo""#, &["bar"]))
        .build();

    let baz = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("baz"))
            .file("src/lib.rs", "")
    });

    let bar_toml = format!(
        r#"[package]
        name = "bar"
        version = "0.0.1"
        authors = []

        [dependencies.baz]
        git = "{}"
        "#,
        baz.url()
    );
    project()
        .at("bar")
        .file("Cargo.toml", &bar_toml)
        .file("src/lib.rs", &main_file(r#""I am bar""#, &["baz"]))
        .build();

    // First, only build remote dependencies; this should only build
    // `baz` but not `bar` and `foo`:
    p.cargo("build -Z unstable-options --remote-dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();

    assert!(p.dep_built("baz"));
    assert!(!p.dep_built("bar"));
    assert!(!p.bin("foo").is_file());

    // Second, build all dependencies; this should only build
    // `bar` as `baz` should already be built and `foo` still
    // should not be built:
    p.cargo("build -Z unstable-options --dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();

    assert!(p.dep_built("baz"));
    assert!(p.dep_built("bar"));
    assert!(!p.bin("foo").is_file());

    // Finally, build all. This should build `foo` since
    // `bar` and `baz` should already be cached
    p.cargo("build")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();

    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn build_deps_features() {
    // Setup a project with features & optional dependencies
    // foo -> bar (path, feature use_bar)
    // foo -> baz (git, feature use_baz)

    let baz = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("baz"))
            .file("src/lib.rs", "")
    });

    let p_toml = format!(
        r#"[package]
        name = "foo"
        version = "0.0.1"
        authors = []

        [features]
        use_bar = ["bar"]
        use_baz = ["baz"]

        [dependencies]
        bar = {{ path = "../bar", optional = true }}
        baz = {{ git = "{}", optional = true }}
        "#,
        baz.url()
    );
    let p_main = r#"
        #[cfg(use_bar)] extern crate bar;
        #[cfg(use_baz)] extern crate baz;

        pub fn main() {
            println!("I am foo");
        }
        "#;
    let p = project()
        .file("Cargo.toml", &p_toml)
        .file("src/main.rs", p_main)
        .build();

    project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"[package]
            name = "bar"
            version = "0.0.1"
            authors = []
            "#,
        )
        .file("src/lib.rs", &main_file(r#""i am bar""#, &[]))
        .build();

    // Build with no features turned on, check that building
    // dependencies doesn't build them
    p.cargo("build -Z unstable-options --remote-dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();
    p.cargo("build -Z unstable-options --dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();
    p.cargo("build")
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();
    assert!(!p.dep_built("bar"));
    assert!(!p.dep_built("baz"));
    assert!(p.bin("foo").is_file());
    p.cargo("clean").run();

    // Build with use_bar, check bar is built in the --dependencies step
    p.cargo("build --features use_bar -Z unstable-options --remote-dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();
    p.cargo("build --features use_bar -Z unstable-options --dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();
    p.cargo("build --features use_bar")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();
    assert!(p.dep_built("bar"));
    assert!(!p.dep_built("baz"));
    assert!(p.bin("foo").is_file());
    p.cargo("clean").run();

    // Build with use_baz, check baz is built in the --remote-dependencies step
    p.cargo("build --features use_baz -Z unstable-options --remote-dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();
    p.cargo("build --features use_baz -Z unstable-options --dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .run();
    p.cargo("build --features use_baz")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();
    assert!(!p.dep_built("bar"));
    assert!(p.dep_built("baz"));
    assert!(p.bin("foo").is_file());
    p.cargo("clean").run();
}

#[cargo_test]
fn build_deps_complex() {
    // Setup a more complex project with workspace members,
    // local dependencies, git dependencies, registry dependencies,
    // and patches.
    //
    // The structure is as follows:
    // workspace: [foo, foo1, foo2]
    //     - foo ----------> foo1
    //          `--> foo2 ----^
    // dependencies:
    //     - foo -> bar (path) -> baz (path) -------------v
    //     - foo1 -> dex (registry) -> hex (registry) -> qux (registry)
    //     - foo2 -> ook (git) ---------------------------^
    //     - foo2 -> eek (registry)
    // pathces:
    //     - hex
    //     - qux
    //     - eek
    //
    // What is verified here:
    //   1. That --remote-dependencies works, ie. builds git and registry
    //      dependencies but not workspace packages and local (path) deps.
    //   2. That --dependencies works, ie. build all dependencies, but not
    //      packages in the workspace
    //   3. That --remote-dependencies treats patches reasonably, ie.
    //      it builds `hex` and `qux` since a remote dependency `dex` needs them,
    //      but not `eek`, since no remote package needs it.
    //   4. That --dependencies treats patches reasonably, ie. builds all patched
    //      dependencies.

    // Registry packages:
    registry::Package::new("qux", "0.5.0").publish();
    registry::Package::new("hex", "0.5.0")
        .dep("qux", "0.5.0")
        .publish();
    registry::Package::new("dex", "0.5.0")
        .dep("hex", "0.5.0")
        .publish();
    registry::Package::new("eek", "0.5.0").publish();

    // Git packages:
    let ook = git::new("ook", |project| {
        let ook_toml = r#"[package]
            name = "ook"
            version = "0.5.0"
            authors = []

            [dependencies]
            qux = "0.5.0"
            "#;
        project
            .file("Cargo.toml", ook_toml)
            .file("src/lib.rs", "extern crate qux;")
    });

    // Patch packages:
    project()
        .at("hex")
        .file(
            "Cargo.toml",
            r#"[package]
            name = "hex"
            version = "0.5.0"
            authors = []

            [dependencies]
            qux = "0.5.0"
            "#,
        )
        .file("src/lib.rs", &main_file(r#""I am hex""#, &["qux"]))
        .build();
    project()
        .at("qux")
        .file("Cargo.toml", &basic_lib_manifest("qux"))
        .file("src/lib.rs", &main_file(r#""I am qux""#, &[]))
        .build();
    project()
        .at("eek")
        .file("Cargo.toml", &basic_lib_manifest("eek"))
        .file("src/lib.rs", &main_file(r#""I am eek""#, &[]))
        .build();

    // Local non-workspace packages:
    project()
        .at("baz")
        .file(
            "Cargo.toml",
            r#"[package]
            name = "baz"
            version = "0.5.0"
            authors = []

            [dependencies]
            qux = "0.5.0"
            "#,
        )
        .file("src/lib.rs", &main_file(r#""I am baz""#, &["qux"]))
        .build();
    project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"[package]
            name = "bar"
            version = "0.5.0"
            authors = []

            [dependencies]
            baz = { path = "../baz" }
            "#,
        )
        .file("src/lib.rs", &main_file(r#""I am bar""#, &["baz"]))
        .build();

    // Workspace:
    let foo2_toml = format!(
        r#"[package]
        name = "foo2"
        version = "0.1.0"
        authors = []

        [dependencies]
        foo1 = {{ path = "../foo1" }}
        ook = {{ git = "{}" }}
        eek = "0.5.0"
        "#,
        ook.url()
    );
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name = "foo"

            [dependencies]
            foo1 = { path = "foo1" }
            foo2 = { path = "foo2" }
            bar = { path = "../bar" }

            [workspace]
            members = ["foo1", "foo2"]

            [patch.crates-io]
            hex = { path = "../hex" }
            qux = { path = "../qux" }
            eek = { path = "../eek" }
        "#,
        )
        .file("src/main.rs", &main_file(r#""I am foo""#, &["foo1", "foo2"]))
        .file(
            "foo1/Cargo.toml",
            r#"[package]
            name = "foo1"
            version = "0.1.0"
            authors = []

            [dependencies]
            dex = "0.5.0"
            "#,
        )
        .file("foo1/src/lib.rs", &main_file(r#""I am foo1""#, &["dex"]))
        .file("foo2/Cargo.toml", &foo2_toml)
        .file(
            "foo2/src/lib.rs",
            &main_file(r#""I am foo2""#, &["ook", "eek"]),
        )
        .build();

    // Build:

    // First, only build remote dependencies
    p.cargo("build -Z unstable-options --remote-dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_contains("[COMPILING] dex [..]")
        .with_stderr_contains("[COMPILING] hex [..]")
        .with_stderr_contains("[COMPILING] qux [..]")
        .with_stderr_contains("[COMPILING] ook [..]")
        .with_stderr_does_not_contain("[COMPILING] eek [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_does_not_contain("[COMPILING] foo1 [..]")
        .with_stderr_does_not_contain("[COMPILING] foo2 [..]")
        .run();

    assert!(!p.dep_built("bar"));
    assert!(!p.dep_built("baz"));
    assert!(p.dep_built("dex"));
    assert!(p.dep_built("hex"));
    assert!(p.dep_built("qux"));
    assert!(p.dep_built("ook"));
    assert!(!p.dep_built("eek"));
    assert!(!p.dep_built("foo1"));
    assert!(!p.dep_built("foo2"));
    assert!(!p.bin("foo").is_file());

    // Second, build all dependencies
    // (remote ones should now be cached)
    p.cargo("build -Z unstable-options --dependencies")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] dex [..]")
        .with_stderr_does_not_contain("[COMPILING] hex [..]")
        .with_stderr_does_not_contain("[COMPILING] qux [..]")
        .with_stderr_does_not_contain("[COMPILING] ook [..]")
        .with_stderr_contains("[COMPILING] eek [..]")
        .with_stderr_does_not_contain("[COMPILING] foo [..]")
        .with_stderr_does_not_contain("[COMPILING] foo1 [..]")
        .with_stderr_does_not_contain("[COMPILING] foo2 [..]")
        .run();

    assert!(p.dep_built("bar"));
    assert!(p.dep_built("baz"));
    assert!(p.dep_built("dex"));
    assert!(p.dep_built("hex"));
    assert!(p.dep_built("qux"));
    assert!(p.dep_built("ook"));
    assert!(p.dep_built("eek"));
    assert!(!p.dep_built("foo1"));
    assert!(!p.dep_built("foo2"));
    assert!(!p.bin("foo").is_file());

    // Finally, build all. This should build the workspace
    // packages, the dependencies should be cached.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[COMPILING] bar [..]")
        .with_stderr_does_not_contain("[COMPILING] baz [..]")
        .with_stderr_does_not_contain("[COMPILING] dex [..]")
        .with_stderr_does_not_contain("[COMPILING] hex [..]")
        .with_stderr_does_not_contain("[COMPILING] qux [..]")
        .with_stderr_does_not_contain("[COMPILING] ook [..]")
        .with_stderr_does_not_contain("[COMPILING] eek [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] foo1 [..]")
        .with_stderr_contains("[COMPILING] foo2 [..]")
        .run();

    assert!(p.dep_built("bar"));
    assert!(p.dep_built("baz"));
    assert!(p.dep_built("dex"));
    assert!(p.dep_built("hex"));
    assert!(p.dep_built("qux"));
    assert!(p.dep_built("ook"));
    assert!(p.dep_built("eek"));
    assert!(p.dep_built("foo1"));
    assert!(p.dep_built("foo2"));
    assert!(p.bin("foo").is_file());
}
