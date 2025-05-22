//! Tests for directory sources.

use std::collections::HashMap;
use std::fs;
use std::str;

use cargo_test_support::cargo_process;
use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::{cksum, Package};
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, project, t, ProjectBuilder};
use serde::Serialize;

fn setup() {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [source.crates-io]
            replace-with = 'my-awesome-local-registry'

            [source.my-awesome-local-registry]
            directory = 'index'
        "#
    ));
}

struct VendorPackage {
    p: Option<ProjectBuilder>,
    cksum: Checksum,
}

#[derive(Serialize)]
struct Checksum {
    package: Option<String>,
    files: HashMap<String, String>,
}

impl VendorPackage {
    fn new(name: &str) -> VendorPackage {
        VendorPackage {
            p: Some(project().at(&format!("index/{}", name))),
            cksum: Checksum {
                package: Some(String::new()),
                files: HashMap::new(),
            },
        }
    }

    fn file(&mut self, name: &str, contents: &str) -> &mut VendorPackage {
        self.p = Some(self.p.take().unwrap().file(name, contents));
        self.cksum
            .files
            .insert(name.to_string(), cksum(contents.as_bytes()));
        self
    }

    fn disable_checksum(&mut self) -> &mut VendorPackage {
        self.cksum.package = None;
        self
    }

    fn no_manifest(mut self) -> Self {
        self.p = self.p.map(|pb| pb.no_manifest());
        self
    }

    fn build(&mut self) {
        let p = self.p.take().unwrap();
        let json = serde_json::to_string(&self.cksum).unwrap();
        let p = p.file(".cargo-checksum.json", &json);
        let _ = p.build();
    }
}

#[cargo_test]
fn simple() {
    setup();

    VendorPackage::new("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn simple_install() {
    setup();

    VendorPackage::new("foo")
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    VendorPackage::new("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.0.1"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate foo; pub fn main() { foo::foo(); }",
        )
        .build();

    cargo_process("install bar")
        .with_stderr_data(str![[r#"
[INSTALLING] bar v0.1.0
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1
[COMPILING] bar v0.1.0
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [..]bar[..]
[INSTALLED] package `bar v0.1.0` (executable `bar[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn simple_install_fail() {
    setup();

    VendorPackage::new("foo")
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    VendorPackage::new("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.1.0"
                baz = "9.8.7"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate foo; pub fn main() { foo::foo(); }",
        )
        .build();

    cargo_process("install bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[INSTALLING] bar v0.1.0
[ERROR] failed to compile `bar v0.1.0`, intermediate artifacts can be found at `[..]`.
To reuse those artifacts with a future compilation, set the environment variable `CARGO_TARGET_DIR` to that path.

Caused by:
  no matching package found
  searched package name: `baz`
  perhaps you meant:      bar or foo
  location searched: directory source `[ROOT]/index` (which is replacing registry `crates-io`)
  required by package `bar v0.1.0`

"#]])
        .run();
}

#[cargo_test]
fn install_without_feature_dep() {
    setup();

    VendorPackage::new("foo")
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    VendorPackage::new("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.0.1"
                baz = { version = "9.8.7", optional = true }

                [features]
                wantbaz = ["baz"]
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate foo; pub fn main() { foo::foo(); }",
        )
        .build();

    cargo_process("install bar")
        .with_stderr_data(str![[r#"
[INSTALLING] bar v0.1.0
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1
[COMPILING] bar v0.1.0
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [..]bar[..]
[INSTALLED] package `bar v0.1.0` (executable `bar[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn not_there() {
    setup();

    let _ = project().at("index").build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package named `bar` found
location searched: directory source `[ROOT]/index` (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
fn multiple() {
    setup();

    VendorPackage::new("bar-0.1.0")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .file(".cargo-checksum", "")
        .build();

    VendorPackage::new("bar-0.2.0")
        .file("Cargo.toml", &basic_manifest("bar", "0.2.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .file(".cargo-checksum", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ADDING] bar v0.1.0 (available: v0.2.0)
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn crates_io_then_directory() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    let cksum = Package::new("bar", "0.1.0")
        .file("src/lib.rs", "pub fn bar() -> u32 { 0 }")
        .publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    setup();

    let mut v = VendorPackage::new("bar");
    v.file("Cargo.toml", &basic_manifest("bar", "0.1.0"));
    v.file("src/lib.rs", "pub fn bar() -> u32 { 1 }");
    v.cksum.package = Some(cksum);
    v.build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn crates_io_then_bad_checksum() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("check").run();
    setup();

    VendorPackage::new("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] checksum for `bar v0.1.0` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g., a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `bar v0.1.0` is the same as when the lockfile was generated


"#]])
        .run();
}

#[cargo_test]
fn bad_file_checksum() {
    setup();

    VendorPackage::new("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    t!(fs::write(
        paths::root().join("index/bar/src/lib.rs"),
        "fn bar() -> u32 { 0 }"
    ));

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] the listed checksum of `[ROOT]/index/bar/src/lib.rs` has changed:
expected: [..]
actual:   [..]

directory sources are not intended to be edited, if modifications are required then it is recommended that `[patch]` is used with a forked copy of the source

"#]])
        .run();
}

#[cargo_test]
fn only_dot_files_ok() {
    setup();

    VendorPackage::new("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();
    VendorPackage::new("foo")
        .no_manifest()
        .file(".bar", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn random_files_ok() {
    setup();

    VendorPackage::new("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();
    VendorPackage::new("foo")
        .no_manifest()
        .file("bar", "")
        .file("../test", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn git_lock_file_doesnt_change() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", &basic_manifest("git", "0.5.0"))
            .file("src/lib.rs", "")
    });

    VendorPackage::new("git")
        .file("Cargo.toml", &basic_manifest("git", "0.5.0"))
        .file("src/lib.rs", "")
        .disable_checksum()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    git = {{ git = '{0}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    let lock1 = p.read_lockfile();

    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        format!(
            r#"
                [source.my-git-repo]
                git = '{}'
                replace-with = 'my-awesome-local-registry'

                [source.my-awesome-local-registry]
                directory = 'index'
            "#,
            git.url()
        )
    ));

    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] git v0.5.0 ([..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lock2 = p.read_lockfile();
    assert_eq!(lock1, lock2, "lock files changed");
}

#[cargo_test]
fn git_override_requires_lockfile() {
    VendorPackage::new("git")
        .file("Cargo.toml", &basic_manifest("git", "0.5.0"))
        .file("src/lib.rs", "")
        .disable_checksum()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                git = { git = 'https://example.com/' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [source.my-git-repo]
            git = 'https://example.com/'
            replace-with = 'my-awesome-local-registry'

            [source.my-awesome-local-registry]
            directory = 'index'
        "#
    ));

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `git` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `git`

Caused by:
  Unable to update [..]

Caused by:
  the source my-git-repo requires a lock file to be present first before it can be
  used against vendored source code

  remove the source replacement configuration, generate a lock file, and then
  restore the source replacement configuration to continue the build

"#]])
        .run();
}

#[cargo_test]
fn workspace_different_locations() {
    let p = project()
        .no_manifest()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = "2015"

                [dependencies]
                baz = "*"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file("foo/vendor/baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("foo/vendor/baz/src/lib.rs", "")
        .file("foo/vendor/baz/.cargo-checksum.json", "{\"files\":{}}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = 'bar'
                version = '0.1.0'
                edition = "2015"

                [dependencies]
                baz = "*"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                target-dir = './target'

                [source.crates-io]
                replace-with = 'my-awesome-local-registry'

                [source.my-awesome-local-registry]
                directory = 'foo/vendor'
            "#,
        )
        .build();

    p.cargo("check").cwd("foo").run();
    p.cargo("check")
        .cwd("bar")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn version_missing() {
    setup();

    VendorPackage::new("foo")
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    VendorPackage::new("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    cargo_process("install bar")
        .with_stderr_data(str![[r#"
[INSTALLING] bar v0.1.0
[ERROR] failed to compile [..], intermediate artifacts can be found at `[..]`.
To reuse those artifacts with a future compilation, set the environment variable `CARGO_TARGET_DIR` to that path.

Caused by:
  failed to select a version for the requirement `foo = "^2"`
  candidate versions found which didn't match: 0.0.1
  location searched: directory source `[..] (which is replacing registry `[..]`)
  required by package `bar v0.1.0`
  perhaps a crate was updated and forgotten to be re-vendored?

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn root_dir_diagnostics() {
    let p = ProjectBuilder::new(paths::root())
        .no_manifest() // we are placing it in a different dir
        .file(
            "ws_root/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file("ws_root/src/lib.rs", "invalid;")
        .build();

    // Crucially, the rustc error message below says `ws_root/...`, i.e.
    // it is relative to our fake home, not to the workspace root.
    p.cargo("check")
        .arg("-Zroot-dir=.")
        .arg("--manifest-path=ws_root/Cargo.toml")
        .masquerade_as_nightly_cargo(&["-Zroot-dir"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/ws_root)
[ERROR] [..]
 --> ws_root/src/lib.rs:1:8
  |
1 | invalid;
  | [..]

[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn root_dir_file_macro() {
    let p = ProjectBuilder::new(paths::root())
        .no_manifest() // we are placing it in a different dir
        .file(
            "ws_root/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file(
            "ws_root/src/main.rs",
            r#"fn main() { println!("{}", file!()); }"#,
        )
        .build();

    // Crucially, the path is relative to our fake home, not to the workspace root.
    p.cargo("run")
        .arg("-Zroot-dir=.")
        .arg("--manifest-path=ws_root/Cargo.toml")
        .masquerade_as_nightly_cargo(&["-Zroot-dir"])
        .with_stdout_data(str![[r#"
ws_root/src/main.rs

"#]])
        .run();
    // Try again with an absolute path for `root-dir`.
    p.cargo("run")
        .arg(format!("-Zroot-dir={}", p.root().display()))
        .arg("--manifest-path=ws_root/Cargo.toml")
        .masquerade_as_nightly_cargo(&["-Zroot-dir"])
        .with_stdout_data(str![[r#"
ws_root/src/main.rs

"#]])
        .run();
}
