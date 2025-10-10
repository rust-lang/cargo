//! Tests for the `cargo tree` command.

use crate::prelude::*;
use crate::utils::cross_compile::disabled as cross_compile_disabled;
use cargo_test_support::cross_compile::alternate;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::str;
use cargo_test_support::{Project, basic_manifest, git, project, rustc_host};

use crate::features2::switch_to_resolver_2;

fn make_simple_proj() -> Project {
    Package::new("c", "1.0.0").publish();
    Package::new("b", "1.0.0").dep("c", "1.0").publish();
    Package::new("a", "1.0.0").dep("b", "1.0").publish();
    Package::new("bdep", "1.0.0").dep("b", "1.0").publish();
    Package::new("devdep", "1.0.0").dep("b", "1.0.0").publish();

    project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            a = "1.0"
            c = "1.0"

            [build-dependencies]
            bdep = "1.0"

            [dev-dependencies]
            devdep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build()
}

#[cargo_test]
fn simple() {
    // A simple test with a few different dependencies.
    let p = make_simple_proj();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
│   └── b v1.0.0
│       └── c v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── b v1.0.0 (*)
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0 (*)

"#]])
        .run();

    p.cargo("tree -p bdep")
        .with_stdout_data(str![[r#"
bdep v1.0.0
└── b v1.0.0
    └── c v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn virtual_workspace() {
    // Multiple packages in a virtual workspace.
    Package::new("somedep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "baz", "c"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "1.0.0"))
        .file("a/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
            [package]
            name = "baz"
            version = "0.1.0"

            [dependencies]
            c = { path = "../c" }
            somedep = "1.0"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .file("c/Cargo.toml", &basic_manifest("c", "1.0.0"))
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
a v1.0.0 ([ROOT]/foo/a)

baz v0.1.0 ([ROOT]/foo/baz)
├── c v1.0.0 ([ROOT]/foo/c)
└── somedep v1.0.0

c v1.0.0 ([ROOT]/foo/c)

"#]])
        .run();

    p.cargo("tree -p a")
        .with_stdout_data(str![[r#"
a v1.0.0 ([ROOT]/foo/a)

"#]])
        .run();

    p.cargo("tree")
        .cwd("baz")
        .with_stdout_data(str![[r#"
baz v0.1.0 ([ROOT]/foo/baz)
├── c v1.0.0 ([ROOT]/foo/c)
└── somedep v1.0.0

"#]])
        .run();

    // exclude baz
    p.cargo("tree --workspace --exclude baz")
        .with_stdout_data(str![[r#"
a v1.0.0 ([ROOT]/foo/a)

c v1.0.0 ([ROOT]/foo/c)

"#]])
        .run();

    // exclude glob '*z'
    p.cargo("tree --workspace --exclude '*z'")
        .with_stdout_data(str![[r#"
a v1.0.0 ([ROOT]/foo/a)

c v1.0.0 ([ROOT]/foo/c)

"#]])
        .run();

    // include glob '*z'
    p.cargo("tree -p '*z'")
        .with_stdout_data(str![[r#"
baz v0.1.0 ([ROOT]/foo/baz)
├── c v1.0.0 ([ROOT]/foo/c)
└── somedep v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn dedupe_edges() {
    // Works around https://github.com/rust-lang/cargo/issues/7985
    Package::new("bitflags", "1.0.0").publish();
    Package::new("manyfeat", "1.0.0")
        .feature("f1", &[])
        .feature("f2", &[])
        .feature("f3", &[])
        .dep("bitflags", "1.0")
        .publish();
    Package::new("a", "1.0.0")
        .feature_dep("manyfeat", "1.0", &["f1"])
        .publish();
    Package::new("b", "1.0.0")
        .feature_dep("manyfeat", "1.0", &["f2"])
        .publish();
    Package::new("c", "1.0.0")
        .feature_dep("manyfeat", "1.0", &["f3"])
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            a = "1.0"
            b = "1.0"
            c = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
│   └── manyfeat v1.0.0
│       └── bitflags v1.0.0
├── b v1.0.0
│   └── manyfeat v1.0.0 (*)
└── c v1.0.0
    └── manyfeat v1.0.0 (*)

"#]])
        .run();
}

#[cargo_test]
fn renamed_deps() {
    // Handles renamed dependencies.
    Package::new("one", "1.0.0").publish();
    Package::new("two", "1.0.0").publish();
    Package::new("bar", "1.0.0").dep("one", "1.0").publish();
    Package::new("bar", "2.0.0").dep("two", "1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"

            [dependencies]
            bar1 = {version = "1.0", package="bar"}
            bar2 = {version = "2.0", package="bar"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
├── bar v1.0.0
│   └── one v1.0.0
└── bar v2.0.0
    └── two v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn source_kinds() {
    // Handles git and path sources.
    Package::new("regdep", "1.0.0").publish();
    let git_project = git::new("gitdep", |p| {
        p.file("Cargo.toml", &basic_manifest("gitdep", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                regdep = "1.0"
                pathdep = {{ path = "pathdep" }}
                gitdep = {{ git = "{}" }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .file("pathdep/Cargo.toml", &basic_manifest("pathdep", "1.0.0"))
        .file("pathdep/src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── gitdep v1.0.0 ([ROOTURL]/gitdep#[..])
├── pathdep v1.0.0 ([ROOT]/foo/pathdep)
└── regdep v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn features() {
    // Exercises a variety of feature behaviors.
    Package::new("optdep_default", "1.0.0").publish();
    Package::new("optdep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"

            [dependencies]
            optdep_default = { version = "1.0", optional = true }
            optdep = { version = "1.0", optional = true }

            [features]
            default = ["optdep_default"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo)
└── optdep_default v1.0.0

"#]])
        .run();

    p.cargo("tree --no-default-features")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree --all-features")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo)
├── optdep v1.0.0
└── optdep_default v1.0.0

"#]])
        .run();

    p.cargo("tree --features optdep")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo)
├── optdep v1.0.0
└── optdep_default v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn filters_target() {
    // --target flag
    if cross_compile_disabled() {
        return;
    }
    Package::new("targetdep", "1.0.0").publish();
    Package::new("hostdep", "1.0.0").publish();
    Package::new("devdep", "1.0.0").publish();
    Package::new("build_target_dep", "1.0.0").publish();
    Package::new("build_host_dep", "1.0.0")
        .target_dep("targetdep", "1.0", alternate())
        .target_dep("hostdep", "1.0", rustc_host())
        .publish();
    Package::new("pm_target", "1.0.0")
        .proc_macro(true)
        .publish();
    Package::new("pm_host", "1.0.0").proc_macro(true).publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [target.'{alt}'.dependencies]
                targetdep = "1.0"
                pm_target = "1.0"

                [target.'{host}'.dependencies]
                hostdep = "1.0"
                pm_host = "1.0"

                [target.'{alt}'.dev-dependencies]
                devdep = "1.0"

                [target.'{alt}'.build-dependencies]
                build_target_dep = "1.0"

                [target.'{host}'.build-dependencies]
                build_host_dep = "1.0"
                "#,
                alt = alternate(),
                host = rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── hostdep v1.0.0
└── pm_host v1.0.0 (proc-macro)
[build-dependencies]
└── build_host_dep v1.0.0
    └── hostdep v1.0.0

"#]])
        .run();

    p.cargo("tree --target")
        .arg(alternate())
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── pm_target v1.0.0 (proc-macro)
└── targetdep v1.0.0
[build-dependencies]
└── build_host_dep v1.0.0
    └── hostdep v1.0.0
[dev-dependencies]
└── devdep v1.0.0

"#]])
        .run();

    p.cargo("tree --target")
        .arg(rustc_host())
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── hostdep v1.0.0
└── pm_host v1.0.0 (proc-macro)
[build-dependencies]
└── build_host_dep v1.0.0
    └── hostdep v1.0.0

"#]])
        .run();

    p.cargo("tree --target=all")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── hostdep v1.0.0
├── pm_host v1.0.0 (proc-macro)
├── pm_target v1.0.0 (proc-macro)
└── targetdep v1.0.0
[build-dependencies]
├── build_host_dep v1.0.0
│   ├── hostdep v1.0.0
│   └── targetdep v1.0.0
└── build_target_dep v1.0.0
[dev-dependencies]
└── devdep v1.0.0

"#]])
        .run();

    // no-proc-macro
    p.cargo("tree --target=all -e no-proc-macro")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── hostdep v1.0.0
└── targetdep v1.0.0
[build-dependencies]
├── build_host_dep v1.0.0
│   ├── hostdep v1.0.0
│   └── targetdep v1.0.0
└── build_target_dep v1.0.0
[dev-dependencies]
└── devdep v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn no_selected_target_dependency() {
    // --target flag
    if cross_compile_disabled() {
        return;
    }
    Package::new("targetdep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [target.'{alt}'.dependencies]
                targetdep = "1.0"

                "#,
                alt = alternate(),
            ),
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree -i targetdep")
        .with_stderr_data(str![[r#"
[WARNING] nothing to print.

To find dependencies that require specific target platforms, try to use option `--target all` first, and then narrow your search scope accordingly.

"#]])
        .run();
    p.cargo("tree -i targetdep --target all")
        .with_stdout_data(str![[r#"
targetdep v1.0.0
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn dep_kinds() {
    Package::new("inner-devdep", "1.0.0").publish();
    Package::new("inner-builddep", "1.0.0").publish();
    Package::new("inner-normal", "1.0.0").publish();
    Package::new("inner-pm", "1.0.0").proc_macro(true).publish();
    Package::new("inner-buildpm", "1.0.0")
        .proc_macro(true)
        .publish();
    Package::new("normaldep", "1.0.0")
        .dep("inner-normal", "1.0")
        .dev_dep("inner-devdep", "1.0")
        .build_dep("inner-builddep", "1.0")
        .publish();
    Package::new("devdep", "1.0.0")
        .dep("inner-normal", "1.0")
        .dep("inner-pm", "1.0")
        .dev_dep("inner-devdep", "1.0")
        .build_dep("inner-builddep", "1.0")
        .build_dep("inner-buildpm", "1.0")
        .publish();
    Package::new("builddep", "1.0.0")
        .dep("inner-normal", "1.0")
        .dev_dep("inner-devdep", "1.0")
        .build_dep("inner-builddep", "1.0")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            normaldep = "1.0"

            [dev-dependencies]
            devdep = "1.0"

            [build-dependencies]
            builddep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── normaldep v1.0.0
    └── inner-normal v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0
[build-dependencies]
└── builddep v1.0.0
    └── inner-normal v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0
[dev-dependencies]
└── devdep v1.0.0
    ├── inner-normal v1.0.0
    └── inner-pm v1.0.0 (proc-macro)
    [build-dependencies]
    ├── inner-builddep v1.0.0
    └── inner-buildpm v1.0.0 (proc-macro)

"#]])
        .run();

    p.cargo("tree -e no-dev")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── normaldep v1.0.0
    └── inner-normal v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0
[build-dependencies]
└── builddep v1.0.0
    └── inner-normal v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0

"#]])
        .run();

    p.cargo("tree -e normal")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── normaldep v1.0.0
    └── inner-normal v1.0.0

"#]])
        .run();

    p.cargo("tree -e dev,build")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
[build-dependencies]
└── builddep v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0
[dev-dependencies]
└── devdep v1.0.0
    [build-dependencies]
    ├── inner-builddep v1.0.0
    └── inner-buildpm v1.0.0 (proc-macro)

"#]])
        .run();

    p.cargo("tree -e dev,build,no-proc-macro")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
[build-dependencies]
└── builddep v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0
[dev-dependencies]
└── devdep v1.0.0
    [build-dependencies]
    └── inner-builddep v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn cyclic_dev_dep() {
    // Cyclical dev-dependency and inverse flag.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dev-dependencies]
            dev-dep = { path = "dev-dep" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "dev-dep/Cargo.toml",
            r#"
            [package]
            name = "dev-dep"
            version = "0.1.0"

            [dependencies]
            foo = { path=".." }
            "#,
        )
        .file("dev-dep/src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
[dev-dependencies]
└── dev-dep v0.1.0 ([ROOT]/foo/dev-dep)
    └── foo v0.1.0 ([ROOT]/foo) (*)

"#]])
        .run();

    p.cargo("tree --invert foo")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── dev-dep v0.1.0 ([ROOT]/foo/dev-dep)
    [dev-dependencies]
    └── foo v0.1.0 ([ROOT]/foo) (*)

"#]])
        .run();
}

#[cargo_test]
fn invert() {
    Package::new("b1", "1.0.0").dep("c", "1.0").publish();
    Package::new("b2", "1.0.0").dep("d", "1.0").publish();
    Package::new("c", "1.0.0").publish();
    Package::new("d", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            b1 = "1.0"
            b2 = "1.0"
            c = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── b1 v1.0.0
│   └── c v1.0.0
├── b2 v1.0.0
│   └── d v1.0.0
└── c v1.0.0

"#]])
        .run();

    p.cargo("tree --invert c")
        .with_stdout_data(str![[r#"
c v1.0.0
├── b1 v1.0.0
│   └── foo v0.1.0 ([ROOT]/foo)
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn invert_with_build_dep() {
    // -i for a common dependency between normal and build deps.
    Package::new("common", "1.0.0").publish();
    Package::new("bdep", "1.0.0").dep("common", "1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            common = "1.0"

            [build-dependencies]
            bdep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── common v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── common v1.0.0

"#]])
        .run();

    p.cargo("tree -i common")
        .with_stdout_data(str![[r#"
common v1.0.0
├── bdep v1.0.0
│   [build-dependencies]
│   └── foo v0.1.0 ([ROOT]/foo)
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn no_indent() {
    let p = make_simple_proj();

    p.cargo("tree --prefix=none")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
a v1.0.0
b v1.0.0
c v1.0.0
c v1.0.0
bdep v1.0.0
b v1.0.0 (*)
devdep v1.0.0
b v1.0.0 (*)

"#]])
        .run();
}

#[cargo_test]
fn prefix_depth() {
    let p = make_simple_proj();

    p.cargo("tree --prefix=depth")
        .with_stdout_data(str![[r#"
0foo v0.1.0 ([ROOT]/foo)
1a v1.0.0
2b v1.0.0
3c v1.0.0
1c v1.0.0
1bdep v1.0.0
2b v1.0.0 (*)
1devdep v1.0.0
2b v1.0.0 (*)

"#]])
        .run();
}

#[cargo_test]
fn no_dedupe() {
    let p = make_simple_proj();

    p.cargo("tree --no-dedupe")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
│   └── b v1.0.0
│       └── c v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── b v1.0.0
        └── c v1.0.0
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0
        └── c v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn no_dedupe_cycle() {
    // --no-dedupe with a dependency cycle
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dev-dependencies]
            bar = {path = "bar"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"

            [dependencies]
            foo = {path=".."}
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
[dev-dependencies]
└── bar v0.1.0 ([ROOT]/foo/bar)
    └── foo v0.1.0 ([ROOT]/foo) (*)

"#]])
        .run();

    p.cargo("tree --no-dedupe")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
[dev-dependencies]
└── bar v0.1.0 ([ROOT]/foo/bar)
    └── foo v0.1.0 ([ROOT]/foo) (*)

"#]])
        .run();
}

#[cargo_test]
fn duplicates() {
    Package::new("dog", "1.0.0").publish();
    Package::new("dog", "2.0.0").publish();
    Package::new("cat", "1.0.0").publish();
    Package::new("cat", "2.0.0").publish();
    Package::new("dep", "1.0.0")
        .dep("dog", "1.0")
        .dep("cat", "1.0")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"

            [dependencies]
            dog1 = { version = "1.0", package = "dog" }
            dog2 = { version = "2.0", package = "dog" }
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"

            [dependencies]
            dep = "1.0"
            cat = "2.0"
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("tree -p a")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo/a)
├── dog v1.0.0
└── dog v2.0.0

"#]])
        .run();

    p.cargo("tree -p b")
        .with_stdout_data(str![[r#"
b v0.1.0 ([ROOT]/foo/b)
├── cat v2.0.0
└── dep v1.0.0
    ├── cat v1.0.0
    └── dog v1.0.0

"#]])
        .run();

    p.cargo("tree -p a -d")
        .with_stdout_data(str![[r#"
dog v1.0.0
└── a v0.1.0 ([ROOT]/foo/a)

dog v2.0.0
└── a v0.1.0 ([ROOT]/foo/a)

"#]])
        .run();

    p.cargo("tree -p b -d")
        .with_stdout_data(str![[r#"
cat v1.0.0
└── dep v1.0.0
    └── b v0.1.0 ([ROOT]/foo/b)

cat v2.0.0
└── b v0.1.0 ([ROOT]/foo/b)

"#]])
        .run();
}

#[cargo_test]
fn duplicates_with_target() {
    // --target flag
    if cross_compile_disabled() {
        return;
    }
    Package::new("a", "1.0.0").publish();
    Package::new("dog", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            a = "1.0"
            dog = "1.0"

            [build-dependencies]
            a = "1.0"
            dog = "1.0"

            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();
    p.cargo("tree -d").with_stdout_data(str![""]).run();

    p.cargo("tree -d --target")
        .arg(alternate())
        .with_stdout_data(str![""])
        .run();

    p.cargo("tree -d --target")
        .arg(rustc_host())
        .with_stdout_data(str![""])
        .run();

    p.cargo("tree -d --target=all")
        .with_stdout_data(str![""])
        .run();
}

#[cargo_test]
fn duplicates_with_proc_macro() {
    Package::new("dupe-dep", "1.0.0").publish();
    Package::new("dupe-dep", "2.0.0").publish();
    Package::new("proc", "1.0.0")
        .proc_macro(true)
        .dep("dupe-dep", "1.0")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            proc = "1.0"
            dupe-dep = "2.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── dupe-dep v2.0.0
└── proc v1.0.0 (proc-macro)
    └── dupe-dep v1.0.0

"#]])
        .run();

    p.cargo("tree --duplicates")
        .with_stdout_data(str![[r#"
dupe-dep v1.0.0
└── proc v1.0.0 (proc-macro)
    └── foo v0.1.0 ([ROOT]/foo)

dupe-dep v2.0.0
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree --duplicates --edges no-proc-macro")
        .with_stdout_data(str![""])
        .run();
}

#[cargo_test]
fn charset() {
    let p = make_simple_proj();
    p.cargo("tree --charset ascii")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
|-- a v1.0.0
|   `-- b v1.0.0
|       `-- c v1.0.0
`-- c v1.0.0
[build-dependencies]
`-- bdep v1.0.0
    `-- b v1.0.0 (*)
[dev-dependencies]
`-- devdep v1.0.0
    `-- b v1.0.0 (*)

"#]])
        .run();
}

#[cargo_test]
fn format() {
    Package::new("dep", "1.0.0").publish();
    Package::new("other-dep", "1.0.0").publish();

    Package::new("dep_that_is_awesome", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "dep_that_is_awesome"
                version = "1.0.0"

                [lib]
                name = "awesome_dep"
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
            license = "MIT"
            repository = "https://github.com/rust-lang/cargo"

            [dependencies]
            dep = {version="1.0", optional=true}
            other-dep = {version="1.0", optional=true}
            dep_that_is_awesome = {version="1.0", optional=true}


            [features]
            default = ["foo"]
            foo = ["bar"]
            bar = []
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("tree --format <<<{p}>>>")
        .with_stdout_data(str![[r#"
<<<foo v0.1.0 ([ROOT]/foo)>>>

"#]])
        .run();

    p.cargo("tree --format {}")
        .with_stderr_data(str![[r#"
[ERROR] tree format `{}` not valid

Caused by:
  unsupported pattern ``

"#]])
        .with_status(101)
        .run();

    p.cargo("tree --format {p}-{{hello}}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)-{hello}

"#]])
        .run();

    p.cargo("tree --format")
        .arg("{p} {l} {r}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) MIT https://github.com/rust-lang/cargo

"#]])
        .run();

    p.cargo("tree --format")
        .arg("{p} {f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) bar,default,foo

"#]])
        .run();

    p.cargo("tree --all-features --format")
        .arg("{p} [{f}]")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) [bar,default,dep,dep_that_is_awesome,foo,other-dep]
├── dep v1.0.0 []
├── dep_that_is_awesome v1.0.0 []
└── other-dep v1.0.0 []

"#]])
        .run();

    p.cargo("tree")
        .arg("--features=other-dep,dep_that_is_awesome")
        .arg("--format={lib}")
        .with_stdout_data(str![[r#"

├── awesome_dep
└── other_dep

"#]])
        .run();
}

#[cargo_test]
fn dev_dep_feature() {
    // New feature resolver with optional dep
    Package::new("optdep", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("optdep", "1.0").optional(true))
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dev-dependencies]
            bar = { version = "1.0", features = ["optdep"] }

            [dependencies]
            bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Old behavior.
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0
    └── optdep v1.0.0
[dev-dependencies]
└── bar v1.0.0 (*)

"#]])
        .run();

    p.cargo("tree -e normal")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0
    └── optdep v1.0.0

"#]])
        .run();

    // New behavior.
    switch_to_resolver_2(&p);

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0
    └── optdep v1.0.0
[dev-dependencies]
└── bar v1.0.0 (*)

"#]])
        .run();

    p.cargo("tree -e normal")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn host_dep_feature() {
    // New feature resolver with optional build dep
    Package::new("optdep", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("optdep", "1.0").optional(true))
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [build-dependencies]
            bar = { version = "1.0", features = ["optdep"] }

            [dependencies]
            bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    // Old behavior
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0
    └── optdep v1.0.0
[build-dependencies]
└── bar v1.0.0 (*)

"#]])
        .run();

    // -p
    p.cargo("tree -p bar")
        .with_stdout_data(str![[r#"
bar v1.0.0
└── optdep v1.0.0

"#]])
        .run();

    // invert
    p.cargo("tree -i optdep")
        .with_stdout_data(str![[r#"
optdep v1.0.0
└── bar v1.0.0
    └── foo v0.1.0 ([ROOT]/foo)
    [build-dependencies]
    └── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    // New behavior.
    switch_to_resolver_2(&p);

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0
[build-dependencies]
└── bar v1.0.0
    └── optdep v1.0.0

"#]])
        .run();

    p.cargo("tree -p bar")
        .with_stdout_data(str![[r#"
bar v1.0.0

bar v1.0.0
└── optdep v1.0.0

"#]])
        .run();

    p.cargo("tree -i optdep")
        .with_stdout_data(str![[r#"
optdep v1.0.0
└── bar v1.0.0
    [build-dependencies]
    └── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    // Check that -d handles duplicates with features.
    p.cargo("tree -d")
        .with_stdout_data(str![[r#"
bar v1.0.0
└── foo v0.1.0 ([ROOT]/foo)

bar v1.0.0
[build-dependencies]
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn proc_macro_features() {
    // New feature resolver with a proc-macro
    Package::new("optdep", "1.0.0").publish();
    Package::new("somedep", "1.0.0")
        .add_dep(Dependency::new("optdep", "1.0").optional(true))
        .publish();
    Package::new("pm", "1.0.0")
        .proc_macro(true)
        .feature_dep("somedep", "1.0", &["optdep"])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            pm = "1.0"
            somedep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Old behavior
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── pm v1.0.0 (proc-macro)
│   └── somedep v1.0.0
│       └── optdep v1.0.0
└── somedep v1.0.0 (*)

"#]])
        .run();

    // Old behavior + no-proc-macro
    p.cargo("tree -e no-proc-macro")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── somedep v1.0.0
    └── optdep v1.0.0

"#]])
        .run();

    // -p
    p.cargo("tree -p somedep")
        .with_stdout_data(str![[r#"
somedep v1.0.0
└── optdep v1.0.0

"#]])
        .run();

    // -p -e no-proc-macro
    p.cargo("tree -p somedep -e no-proc-macro")
        .with_stdout_data(str![[r#"
somedep v1.0.0
└── optdep v1.0.0

"#]])
        .run();

    // invert
    p.cargo("tree -i somedep")
        .with_stdout_data(str![[r#"
somedep v1.0.0
├── foo v0.1.0 ([ROOT]/foo)
└── pm v1.0.0 (proc-macro)
    └── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    // invert + no-proc-macro
    p.cargo("tree -i somedep -e no-proc-macro")
        .with_stdout_data(str![[r#"
somedep v1.0.0
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    // New behavior.
    switch_to_resolver_2(&p);

    // Note the missing (*)
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── pm v1.0.0 (proc-macro)
│   └── somedep v1.0.0
│       └── optdep v1.0.0
└── somedep v1.0.0

"#]])
        .run();

    p.cargo("tree -e no-proc-macro")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── somedep v1.0.0

"#]])
        .run();

    p.cargo("tree -p somedep")
        .with_stdout_data(str![[r#"
somedep v1.0.0

somedep v1.0.0
└── optdep v1.0.0

"#]])
        .run();

    p.cargo("tree -i somedep")
        .with_stdout_data(str![[r#"
somedep v1.0.0
└── foo v0.1.0 ([ROOT]/foo)

somedep v1.0.0
└── pm v1.0.0 (proc-macro)
    └── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree -i somedep -e no-proc-macro")
        .with_stdout_data(str![[r#"
somedep v1.0.0
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn itarget_opt_dep() {
    // New feature resolver with optional target dep
    Package::new("optdep", "1.0.0").publish();
    Package::new("common", "1.0.0")
        .add_dep(Dependency::new("optdep", "1.0").optional(true))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"

            [dependencies]
            common = "1.0"

            [target.'cfg(whatever)'.dependencies]
            common = { version = "1.0", features = ["optdep"] }

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Old behavior
    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
└── common v1.0.0
    └── optdep v1.0.0

"#]])
        .run();

    // New behavior.
    switch_to_resolver_2(&p);

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
└── common v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn ambiguous_name() {
    // -p that is ambiguous.
    Package::new("dep", "1.0.0").publish();
    Package::new("dep", "2.0.0").publish();
    Package::new("bar", "1.0.0").dep("dep", "2.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = "1.0"
            bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -p dep")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[ADDING] dep v1.0.0 (available: v2.0.0)
[DOWNLOADING] crates ...
[DOWNLOADED] dep v2.0.0 (registry `dummy-registry`)
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[ERROR] There are multiple `dep` packages in your project, and the specification `dep` is ambiguous.
Please re-run this command with one of the following specifications:
  dep@1.0.0
  dep@2.0.0

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn workspace_features_are_local() {
    // The features for workspace packages should be the same as `cargo build`
    // (i.e., the features selected depend on the "current" package).
    Package::new("optdep", "1.0.0").publish();
    Package::new("somedep", "1.0.0")
        .add_dep(Dependency::new("optdep", "1.0").optional(true))
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"

            [dependencies]
            somedep = {version="1.0", features=["optdep"]}
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"

            [dependencies]
            somedep = "1.0"

            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo/a)
└── somedep v1.0.0
    └── optdep v1.0.0

b v0.1.0 ([ROOT]/foo/b)
└── somedep v1.0.0 (*)

"#]])
        .run();

    p.cargo("tree -p a")
        .with_stdout_data(str![[r#"
a v0.1.0 ([ROOT]/foo/a)
└── somedep v1.0.0
    └── optdep v1.0.0

"#]])
        .run();

    p.cargo("tree -p b")
        .with_stdout_data(str![[r#"
b v0.1.0 ([ROOT]/foo/b)
└── somedep v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn unknown_edge_kind() {
    let p = project()
        .file("Cargo.toml", "")
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e unknown")
        .with_stderr_data(str![[r#"
[ERROR] unknown edge kind `unknown`, valid values are "normal", "build", "dev", "no-normal", "no-build", "no-dev", "no-proc-macro", "features", or "all"

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn mixed_no_edge_kinds() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e no-build,normal")
        .with_stderr_data(str![[r#"
[ERROR] `normal` dependency kind cannot be mixed with "no-normal", "no-build", or "no-dev" dependency kinds

"#]])
        .with_status(101)
        .run();

    // `no-proc-macro` can be mixed with others
    p.cargo("tree -e no-proc-macro,normal")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn depth_limit() {
    let p = make_simple_proj();

    p.cargo("tree --depth 0")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
[build-dependencies]
[dev-dependencies]

"#]])
        .run();

    p.cargo("tree --depth 1")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0
[dev-dependencies]
└── devdep v1.0.0

"#]])
        .run();

    p.cargo("tree --depth 2")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
│   └── b v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── b v1.0.0 (*)
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0 (*)

"#]])
        .run();

    // specify a package
    p.cargo("tree -p bdep --depth 1")
        .with_stdout_data(str![[r#"
bdep v1.0.0
└── b v1.0.0

"#]])
        .run();

    // different prefix
    p.cargo("tree --depth 1 --prefix depth")
        .with_stdout_data(str![[r#"
0foo v0.1.0 ([ROOT]/foo)
1a v1.0.0
1c v1.0.0
1bdep v1.0.0
1devdep v1.0.0

"#]])
        .run();

    // with edge-kinds
    p.cargo("tree --depth 1 -e no-dev")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0

"#]])
        .run();

    // invert
    p.cargo("tree --depth 1 --invert c")
        .with_stdout_data(str![[r#"
c v1.0.0
├── b v1.0.0
└── foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();
}

#[cargo_test]
fn depth_workspace() {
    Package::new("somedep", "1.0.0").publish();
    Package::new("otherdep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b", "c"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "1.0.0"))
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"

            [dependencies]
            c = { path = "../c" }
            somedep = "1"
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "c/Cargo.toml",
            r#"
            [package]
            name = "c"
            version = "0.1.0"

            [dependencies]
            somedep = "1"
            otherdep = "1"
            "#,
        )
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("tree --depth workspace")
        .with_stdout_data(str![[r#"
a v1.0.0 ([ROOT]/foo/a)

b v0.1.0 ([ROOT]/foo/b)
└── c v0.1.0 ([ROOT]/foo/c)

c v0.1.0 ([ROOT]/foo/c) (*)

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn edge_public() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["diamond", "left-pub", "right-priv", "dep"]
            "#,
        )
        .file(
            "diamond/Cargo.toml",
            r#"
            cargo-features = ["public-dependency"]

            [package]
            name = "diamond"
            version = "0.1.0"

            [dependencies]
            left-pub = { path = "../left-pub", public = true }
            right-priv = { path = "../right-priv", public = true }
            "#,
        )
        .file("diamond/src/lib.rs", "")
        .file(
            "left-pub/Cargo.toml",
            r#"
            cargo-features = ["public-dependency"]

            [package]
            name = "left-pub"
            version = "0.1.0"

            [dependencies]
            dep = { path = "../dep", public = true }
            "#,
        )
        .file("left-pub/src/lib.rs", "")
        .file(
            "right-priv/Cargo.toml",
            r#"
            [package]
            name = "right-priv"
            version = "0.1.0"

            [dependencies]
            dep = { path = "../dep" }
            "#,
        )
        .file("right-priv/src/lib.rs", "")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("tree --edges public")
        .masquerade_as_nightly_cargo(&["public-dependency", "edge-public"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `--edges public` requires `-Zunstable-options`

"#]])
        .run();

    p.cargo("tree --edges public -p left-pub")
        .arg("-Zunstable-options")
        .masquerade_as_nightly_cargo(&["public-dependency", "edge-public"])
        .with_stdout_data(str![[r#"
left-pub v0.1.0 ([ROOT]/foo/left-pub)
└── dep v0.1.0 ([ROOT]/foo/dep)

"#]])
        .run();

    p.cargo("tree --edges public -p right-priv")
        .arg("-Zunstable-options")
        .masquerade_as_nightly_cargo(&["public-dependency", "edge-public"])
        .with_stdout_data(str![[r#"
right-priv v0.1.0 ([ROOT]/foo/right-priv)

"#]])
        .run();

    p.cargo("tree --edges public -p diamond")
        .arg("-Zunstable-options")
        .masquerade_as_nightly_cargo(&["public-dependency", "edge-public"])
        .with_stdout_data(str![[r#"
diamond v0.1.0 ([ROOT]/foo/diamond)
├── left-pub v0.1.0 ([ROOT]/foo/left-pub)
│   └── dep v0.1.0 ([ROOT]/foo/dep)
└── right-priv v0.1.0 ([ROOT]/foo/right-priv)

"#]])
        .run();

    p.cargo("tree --edges public")
        .arg("-Zunstable-options")
        .masquerade_as_nightly_cargo(&["public-dependency", "edge-public"])
        .with_stdout_data(str![[r#"
dep v0.1.0 ([ROOT]/foo/dep)

diamond v0.1.0 ([ROOT]/foo/diamond)
├── left-pub v0.1.0 ([ROOT]/foo/left-pub)
│   └── dep v0.1.0 ([ROOT]/foo/dep)
└── right-priv v0.1.0 ([ROOT]/foo/right-priv)

left-pub v0.1.0 ([ROOT]/foo/left-pub) (*)

right-priv v0.1.0 ([ROOT]/foo/right-priv)

"#]])
        .run();

    p.cargo("tree --edges public --invert dep")
        .arg("-Zunstable-options")
        .masquerade_as_nightly_cargo(&["public-dependency", "edge-public"])
        .with_stdout_data(str![[r#"
dep v0.1.0 ([ROOT]/foo/dep)
└── left-pub v0.1.0 ([ROOT]/foo/left-pub)
    └── diamond v0.1.0 ([ROOT]/foo/diamond)

"#]])
        .run();
}

#[cargo_test]
fn prune() {
    let p = make_simple_proj();

    p.cargo("tree --prune c")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── a v1.0.0
    └── b v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── b v1.0.0 (*)
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0 (*)

"#]])
        .run();

    // multiple prune
    p.cargo("tree --prune c --prune bdep")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── a v1.0.0
    └── b v1.0.0
[build-dependencies]
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0 (*)

"#]])
        .run();

    // with edge-kinds
    p.cargo("tree --prune c -e normal")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── a v1.0.0
    └── b v1.0.0

"#]])
        .run();

    // pruning self does not works
    p.cargo("tree --prune foo")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── a v1.0.0
│   └── b v1.0.0
│       └── c v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── b v1.0.0 (*)
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0 (*)

"#]])
        .run();

    // dep not exist
    p.cargo("tree --prune no-dep")
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `no-dep` did not match any packages

[HELP] a package with a similar name exists: `bdep`

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn cyclic_features() {
    // Check for stack overflow with cyclic features (oops!).
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"

                [features]
                a = ["b"]
                b = ["a"]
                default = ["a"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree -e features -i foo")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
├── foo feature "a"
│   ├── foo feature "b"
│   │   └── foo feature "a" (*)
│   └── foo feature "default" (command-line)
├── foo feature "b" (*)
└── foo feature "default" (command-line)

"#]])
        .run();
}

#[cargo_test]
fn dev_dep_cycle_with_feature() {
    // Cycle with features and a dev-dependency.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"

                [dev-dependencies]
                bar = { path = "bar" }

                [features]
                a = ["bar/feat1"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "1.0.0"

                [dependencies]
                foo = { path = ".." }

                [features]
                feat1 = ["foo/a"]
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("tree -e features --features a")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
[dev-dependencies]
└── bar feature "default"
    └── bar v1.0.0 ([ROOT]/foo/bar)
        └── foo feature "default" (command-line)
            └── foo v1.0.0 ([ROOT]/foo) (*)

"#]])
        .run();

    p.cargo("tree -e features --features a -i foo")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
├── foo feature "a" (command-line)
│   └── bar feature "feat1"
│       └── foo feature "a" (command-line) (*)
└── foo feature "default" (command-line)
    └── bar v1.0.0 ([ROOT]/foo/bar)
        ├── bar feature "default"
        │   [dev-dependencies]
        │   └── foo v1.0.0 ([ROOT]/foo) (*)
        └── bar feature "feat1" (*)

"#]])
        .run();
}

#[cargo_test]
fn dev_dep_cycle_with_feature_nested() {
    // Checks for an issue where a cyclic dev dependency tries to activate a
    // feature on its parent that tries to activate the feature back on the
    // dev-dependency.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"

                [dev-dependencies]
                bar = { path = "bar" }

                [features]
                a = ["bar/feat1"]
                b = ["a"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "1.0.0"

                [dependencies]
                foo = { path = ".." }

                [features]
                feat1 = ["foo/b"]
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("tree -e features")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
[dev-dependencies]
└── bar feature "default"
    └── bar v1.0.0 ([ROOT]/foo/bar)
        └── foo feature "default" (command-line)
            └── foo v1.0.0 ([ROOT]/foo) (*)

"#]])
        .run();

    p.cargo("tree -e features --features a -i foo")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
├── foo feature "a" (command-line)
│   └── foo feature "b"
│       └── bar feature "feat1"
│           └── foo feature "a" (command-line) (*)
├── foo feature "b" (*)
└── foo feature "default" (command-line)
    └── bar v1.0.0 ([ROOT]/foo/bar)
        ├── bar feature "default"
        │   [dev-dependencies]
        │   └── foo v1.0.0 ([ROOT]/foo) (*)
        └── bar feature "feat1" (*)

"#]])
        .run();

    p.cargo("tree -e features --features b -i foo")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
├── foo feature "a"
│   └── foo feature "b" (command-line)
│       └── bar feature "feat1"
│           └── foo feature "a" (*)
├── foo feature "b" (command-line) (*)
└── foo feature "default" (command-line)
    └── bar v1.0.0 ([ROOT]/foo/bar)
        ├── bar feature "default"
        │   [dev-dependencies]
        │   └── foo v1.0.0 ([ROOT]/foo) (*)
        └── bar feature "feat1" (*)

"#]])
        .run();

    p.cargo("tree -e features --features bar/feat1 -i foo")
        .with_stdout_data(str![[r#"
foo v1.0.0 ([ROOT]/foo)
├── foo feature "a"
│   └── foo feature "b"
│       └── bar feature "feat1" (command-line)
│           └── foo feature "a" (*)
├── foo feature "b" (*)
└── foo feature "default" (command-line)
    └── bar v1.0.0 ([ROOT]/foo/bar)
        ├── bar feature "default"
        │   [dev-dependencies]
        │   └── foo v1.0.0 ([ROOT]/foo) (*)
        └── bar feature "feat1" (command-line) (*)

"#]])
        .run();
}

#[cargo_test]
fn no_proc_macro_order() {
    Package::new("dep", "1.0.0").publish();
    Package::new("pm", "1.0.0").proc_macro(true).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            pm = "1.0"
            dep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── dep v1.0.0
└── pm v1.0.0 (proc-macro)

"#]])
        .run();

    // no-proc-macro combined with other edge kinds
    p.cargo("tree -e normal,no-proc-macro")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── dep v1.0.0

"#]])
        .run();

    // change flag order, expecting the same output
    p.cargo("tree -e no-proc-macro,normal")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── dep v1.0.0

"#]])
        .run();
}
