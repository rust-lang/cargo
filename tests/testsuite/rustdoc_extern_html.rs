//! Tests for the -Zrustdoc-map feature.

use cargo_test_support::registry::{self, Package};
use cargo_test_support::{paths, project, Project};

fn basic_project() -> Project {
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "pub struct Straw;")
        .publish();

    project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn myfun() -> Option<bar::Straw> {
                    None
                }
            "#,
        )
        .build()
}

#[cargo_test]
fn ignores_on_stable() {
    // Requires -Zrustdoc-map to use.
    let p = basic_project();
    p.cargo("doc -v --no-deps")
        .with_stderr_does_not_contain("[..]--extern-html-root-url[..]")
        .run();
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn simple() {
    // Basic test that it works with crates.io.
    let p = basic_project();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo [..]bar=https://docs.rs/bar/1.0.0/[..]",
        )
        .run();
    let myfun = p.read_file("target/doc/foo/fn.myfun.html");
    assert!(myfun.contains(r#"href="https://docs.rs/bar/1.0.0/bar/struct.Straw.html""#));
}

#[ignore = "Broken, temporarily disabled until https://github.com/rust-lang/rust/pull/82776 is resolved."]
#[cargo_test]
// #[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn std_docs() {
    // Mapping std docs somewhere else.
    // For local developers, skip this test if docs aren't installed.
    let docs = std::path::Path::new(&paths::sysroot()).join("share/doc/rust/html");
    if !docs.exists() {
        if cargo_util::is_ci() {
            panic!("std docs are not installed, check that the rust-docs component is installed");
        } else {
            eprintln!(
                "documentation not found at {}, \
                skipping test (run `rustdoc component add rust-docs` to install",
                docs.display()
            );
            return;
        }
    }
    let p = basic_project();
    p.change_file(
        ".cargo/config",
        r#"
            [doc.extern-map]
            std = "local"
        "#,
    );
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains("[RUNNING] `rustdoc [..]--crate-name foo [..]std=file://[..]")
        .run();
    let myfun = p.read_file("target/doc/foo/fn.myfun.html");
    assert!(myfun.contains(r#"share/doc/rust/html/core/option/enum.Option.html""#));

    p.change_file(
        ".cargo/config",
        r#"
            [doc.extern-map]
            std = "https://example.com/rust/"
        "#,
    );
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo [..]std=https://example.com/rust/[..]",
        )
        .run();
    let myfun = p.read_file("target/doc/foo/fn.myfun.html");
    assert!(myfun.contains(r#"href="https://example.com/rust/core/option/enum.Option.html""#));
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn renamed_dep() {
    // Handles renamed dependencies.
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "pub struct Straw;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                groovy = { version = "1.0", package = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn myfun() -> Option<groovy::Straw> {
                    None
                }
            "#,
        )
        .build();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo [..]bar=https://docs.rs/bar/1.0.0/[..]",
        )
        .run();
    let myfun = p.read_file("target/doc/foo/fn.myfun.html");
    assert!(myfun.contains(r#"href="https://docs.rs/bar/1.0.0/bar/struct.Straw.html""#));
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn lib_name() {
    // Handles lib name != package name.
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "1.0.0"

                [lib]
                name = "rumpelstiltskin"
            "#,
        )
        .file("src/lib.rs", "pub struct Straw;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn myfun() -> Option<rumpelstiltskin::Straw> {
                    None
                }
            "#,
        )
        .build();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo [..]rumpelstiltskin=https://docs.rs/bar/1.0.0/[..]",
        )
        .run();
    let myfun = p.read_file("target/doc/foo/fn.myfun.html");
    assert!(myfun.contains(r#"href="https://docs.rs/bar/1.0.0/rumpelstiltskin/struct.Straw.html""#));
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn alt_registry() {
    // Supports other registry names.
    registry::alt_init();
    Package::new("bar", "1.0.0")
        .alternative(true)
        .file(
            "src/lib.rs",
            r#"
                extern crate baz;
                pub struct Queen;
                pub use baz::King;
            "#,
        )
        .registry_dep("baz", "1.0")
        .publish();
    Package::new("baz", "1.0.0")
        .alternative(true)
        .file("src/lib.rs", "pub struct King;")
        .publish();
    Package::new("grimm", "1.0.0")
        .file("src/lib.rs", "pub struct Gold;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                bar = { version = "1.0", registry="alternative" }
                grimm = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn queen() -> bar::Queen { bar::Queen }
                pub fn king() -> bar::King { bar::King }
                pub fn gold() -> grimm::Gold { grimm::Gold }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
                [doc.extern-map.registries]
                alternative = "https://example.com/{pkg_name}/{version}/"
                crates-io = "https://docs.rs/"
            "#,
        )
        .build();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo \
            [..]bar=https://example.com/bar/1.0.0/[..]grimm=https://docs.rs/grimm/1.0.0/[..]",
        )
        .run();
    let queen = p.read_file("target/doc/foo/fn.queen.html");
    assert!(queen.contains(r#"href="https://example.com/bar/1.0.0/bar/struct.Queen.html""#));
    // The king example fails to link. Rustdoc seems to want the origin crate
    // name (baz) for re-exports. There are many issues in the issue tracker
    // for rustdoc re-exports, so I'm not sure, but I think this is maybe a
    // rustdoc issue. Alternatively, Cargo could provide mappings for all
    // transitive dependencies to fix this.
    let king = p.read_file("target/doc/foo/fn.king.html");
    assert!(king.contains(r#"-&gt; King"#));

    let gold = p.read_file("target/doc/foo/fn.gold.html");
    assert!(gold.contains(r#"href="https://docs.rs/grimm/1.0.0/grimm/struct.Gold.html""#));
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn multiple_versions() {
    // What happens when there are multiple versions.
    // NOTE: This is currently broken behavior. Rustdoc does not provide a way
    // to match renamed dependencies.
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "pub struct Spin;")
        .publish();
    Package::new("bar", "2.0.0")
        .file("src/lib.rs", "pub struct Straw;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                bar = "1.0"
                bar2 = {version="2.0", package="bar"}
            "#,
        )
        .file(
            "src/lib.rs",
            "
                pub fn fn1() -> bar::Spin {bar::Spin}
                pub fn fn2() -> bar2::Straw {bar2::Straw}
            ",
        )
        .build();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo \
            [..]bar=https://docs.rs/bar/1.0.0/[..]bar=https://docs.rs/bar/2.0.0/[..]",
        )
        .run();
    let fn1 = p.read_file("target/doc/foo/fn.fn1.html");
    // This should be 1.0.0, rustdoc seems to use the last entry when there
    // are duplicates.
    assert!(fn1.contains(r#"href="https://docs.rs/bar/2.0.0/bar/struct.Spin.html""#));
    let fn2 = p.read_file("target/doc/foo/fn.fn2.html");
    assert!(fn2.contains(r#"href="https://docs.rs/bar/2.0.0/bar/struct.Straw.html""#));
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn rebuilds_when_changing() {
    // Make sure it rebuilds if the map changes.
    let p = basic_project();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains("[..]--extern-html-root-url[..]")
        .run();

    // This also tests that the map for docs.rs can be overridden.
    p.change_file(
        ".cargo/config",
        r#"
            [doc.extern-map.registries]
            crates-io = "https://example.com/"
        "#,
    );
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--extern-html-root-url [..]bar=https://example.com/bar/1.0.0/[..]",
        )
        .run();
}

#[cargo_test(nightly, reason = "--extern-html-root-url is unstable")]
fn alt_sparse_registry() {
    // Supports other registry names.

    registry::init();
    let _registry = registry::RegistryBuilder::new()
        .http_index()
        .alternative()
        .build();

    Package::new("bar", "1.0.0")
        .alternative(true)
        .file(
            "src/lib.rs",
            r#"
                extern crate baz;
                pub struct Queen;
                pub use baz::King;
            "#,
        )
        .registry_dep("baz", "1.0")
        .publish();
    Package::new("baz", "1.0.0")
        .alternative(true)
        .file("src/lib.rs", "pub struct King;")
        .publish();
    Package::new("grimm", "1.0.0")
        .file("src/lib.rs", "pub struct Gold;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                bar = { version = "1.0", registry="alternative" }
                grimm = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn queen() -> bar::Queen { bar::Queen }
                pub fn king() -> bar::King { bar::King }
                pub fn gold() -> grimm::Gold { grimm::Gold }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
                [doc.extern-map.registries]
                alternative = "https://example.com/{pkg_name}/{version}/"
                crates-io = "https://docs.rs/"
            "#,
        )
        .build();
    p.cargo("doc -v --no-deps -Zrustdoc-map")
        .masquerade_as_nightly_cargo(&["rustdoc-map"])
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..]--crate-name foo \
            [..]bar=https://example.com/bar/1.0.0/[..]grimm=https://docs.rs/grimm/1.0.0/[..]",
        )
        .run();
    let queen = p.read_file("target/doc/foo/fn.queen.html");
    assert!(queen.contains(r#"href="https://example.com/bar/1.0.0/bar/struct.Queen.html""#));
    // The king example fails to link. Rustdoc seems to want the origin crate
    // name (baz) for re-exports. There are many issues in the issue tracker
    // for rustdoc re-exports, so I'm not sure, but I think this is maybe a
    // rustdoc issue. Alternatively, Cargo could provide mappings for all
    // transitive dependencies to fix this.
    let king = p.read_file("target/doc/foo/fn.king.html");
    assert!(king.contains(r#"-&gt; King"#));

    let gold = p.read_file("target/doc/foo/fn.gold.html");
    assert!(gold.contains(r#"href="https://docs.rs/grimm/1.0.0/grimm/struct.Gold.html""#));
}
