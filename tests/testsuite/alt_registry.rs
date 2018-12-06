use std::fs::File;
use std::io::Write;
use crate::support::registry::{self, alt_api_path, Package};
use crate::support::{basic_manifest, paths, project};

#[test]
fn is_feature_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("  feature `alternative-registries` is required")
        .run();
}

#[test]
fn depend_on_alt_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        )).run();

    p.cargo("clean").masquerade_as_nightly_cargo().run();

    // Don't download a second time
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        ).run();
}

#[test]
fn depend_on_alt_registry_depends_on_same_registry_no_index() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").alternative(true).publish();
    Package::new("bar", "0.0.1")
        .dep("baz", "0.0.1")
        .alternative(true)
        .publish();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        )).run();
}

#[test]
fn depend_on_alt_registry_depends_on_same_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").alternative(true).publish();
    Package::new("bar", "0.0.1")
        .registry_dep("baz", "0.0.1", registry::alt_registry().as_str())
        .alternative(true)
        .publish();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        )).run();
}

#[test]
fn depend_on_alt_registry_depends_on_crates_io() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1")
        .registry_dep("baz", "0.0.1", registry::registry().as_str())
        .alternative(true)
        .publish();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[UPDATING] `{alt_reg}` index
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            alt_reg = registry::alt_registry_path().to_str().unwrap(),
            reg = registry::registry_path().to_str().unwrap()
        )).run();
}

#[test]
fn registry_and_path_dep_works() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        ).run();
}

#[test]
fn registry_incompatible_with_git() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = ""
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().with_status(101)
                .with_stderr_contains("  dependency (bar) specification is ambiguous. Only one of `git` or `registry` is allowed.").run();
}

#[test]
fn cannot_publish_to_crates_io_with_registry_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("publish --index")
        .arg(registry::registry().to_string())
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .run();
}

#[test]
fn publish_with_registry_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    // Login so that we have the token available
    p.cargo("login --registry alternative TOKEN -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();
}

#[test]
fn alt_registry_and_crates_io_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            crates_io_dep = "0.0.1"

            [dependencies.alt_reg_dep]
            version = "0.1.0"
            registry = "alternative"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("crates_io_dep", "0.0.1").publish();
    Package::new("alt_reg_dep", "0.1.0")
        .alternative(true)
        .publish();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(format!(
            "[UPDATING] `{}` index",
            registry::alt_registry_path().to_str().unwrap()
        )).with_stderr_contains(&format!(
            "[UPDATING] `{}` index",
            registry::registry_path().to_str().unwrap()))
        .with_stderr_contains("[DOWNLOADED] crates_io_dep v0.0.1 (registry `[ROOT][..]`)")
        .with_stderr_contains("[DOWNLOADED] alt_reg_dep v0.1.0 (registry `[ROOT][..]`)")
        .with_stderr_contains("[COMPILING] alt_reg_dep v0.1.0 (registry `[ROOT][..]`)")
        .with_stderr_contains("[COMPILING] crates_io_dep v0.0.1")
        .with_stderr_contains("[COMPILING] foo v0.0.1 ([CWD])")
        .with_stderr_contains("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s")
        .run();
}

#[test]
fn block_publish_due_to_no_token() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    // Setup the registry by publishing a package
    Package::new("bar", "0.0.1").alternative(true).publish();

    // Now perform the actual publish
    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("error: no upload token found, please run `cargo login`")
        .run();
}

#[test]
fn publish_to_alt_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    // Setup the registry by publishing a package
    Package::new("bar", "0.0.1").alternative(true).publish();

    // Login so that we have the token available
    p.cargo("login --registry alternative TOKEN -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();

    // Now perform the actual publish
    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();

    // Ensure that the crate is uploaded
    assert!(alt_api_path().join("api/v1/crates/new").exists());
}

#[test]
fn publish_with_crates_io_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = ["me"]
            license = "MIT"
            description = "foo"

            [dependencies.bar]
            version = "0.0.1"
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    // Login so that we have the token available
    p.cargo("login --registry alternative TOKEN -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();
}

#[test]
fn passwords_in_url_forbidden() {
    registry::init();

    let config = paths::home().join(".cargo/config");

    File::create(config)
        .unwrap()
        .write_all(
            br#"
        [registries.alternative]
        index = "ssh://git:secret@foobar.com"
        "#,
        ).unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("error: Registry URLs may not contain passwords")
        .run();
}
