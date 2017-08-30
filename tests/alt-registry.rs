extern crate cargotest;
extern crate hamcrest;

use cargotest::ChannelChanger;
use cargotest::support::registry::{self, Package};
use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn is_feature_gated() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(101)
                .with_stderr_contains("  feature `alternative-registries` is required"));
}

#[test]
fn depend_on_alt_registry() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(0).with_stderr(&format!("\
[UPDATING] registry `{reg}`
[DOWNLOADING] bar v0.0.1 (registry `file://[..]`)
[COMPILING] bar v0.0.1 (registry `file://[..]`)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..] secs
",
        dir = p.url(),
        reg = registry::alt_registry())));

    assert_that(p.cargo("clean").masquerade_as_nightly_cargo(), execs().with_status(0));

    // Don't download a second time
    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] bar v0.0.1 (registry `file://[..]`)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..] secs
",
        dir = p.url())));
}

#[test]
fn registry_incompatible_with_path() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = ""
            registry = "alternative"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(101)
                .with_stderr_contains("  dependency (bar) specification is ambiguous. Only one of `path` or `registry` is allowed."));
}

#[test]
fn registry_incompatible_with_git() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = ""
            registry = "alternative"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(101)
                .with_stderr_contains("  dependency (bar) specification is ambiguous. Only one of `git` or `registry` is allowed."));
}


#[test]
fn cannot_publish_with_registry_dependency() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    assert_that(p.cargo("publish").masquerade_as_nightly_cargo()
                 .arg("--index").arg(registry::alt_registry().to_string()),
                execs().with_status(101));
}
