use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn unused() {
    Package::new("in-package", "1.0.0").publish();
    Package::new("build-dep", "1.0.0").publish();
    Package::new("dep", "1.0.0").publish();
    Package::new("dev-dep", "1.0.0").publish();
    Package::new("target-dep", "1.0.0").publish();
    Package::new("unused", "1.0.0").publish();
    Package::new("not-inherited", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["bar"]

[workspace.dependencies]
in-package = "1"
build-dep = "1"
dep = "1"
dev-dep = "1"
target-dep = "1"
unused = "1"
not-inherited = "1"

[workspace.lints.cargo]
unused_workspace_dependencies = "warn"

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[dependencies]
in-package.workspace = true

[lints]
workspace = true
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"
authors = []

[build-dependencies]
build-dep.workspace = true

[dependencies]
dep.workspace = true
not-inherited = "1"

[dev-dependencies]
dev-dep.workspace = true

[target.'cfg(false)'.dependencies]
target-dep.workspace = true

[lints]
workspace = true
"#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_workspace_dependencies`
  --> Cargo.toml:15:1
   |
15 | unused_workspace_dependencies = "warn"
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 6 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] in-package v1.0.0 (registry `dummy-registry`)
[CHECKING] in-package v1.0.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
