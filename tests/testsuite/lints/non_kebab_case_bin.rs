use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn bin_name_explicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[[bin]]
name = "foo_bar"
path = "src/main.rs"

[lints.cargo]
non_kebab_case_bin = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] binaries should have a kebab-case name
  |
1 | [ROOT]/foo/target/.../foo_bar[EXE]
  |                   [..]^^^^^^^
  |
  = [NOTE] `cargo::non_kebab_case_bin` is set to `warn` in `[lints]`
[HELP] to change the binary name to kebab case, convert `bin.name`
 --> Cargo.toml:9:8
  |
9 - name = "foo_bar"
9 + name = "foo-bar"
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bin_name_from_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo_bar"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
non_kebab_case_bin = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] binaries should have a kebab-case name
   |
 1 | [ROOT]/foo/target/.../foo_bar[EXE]
   |                   [..]^^^^^^^
   |
   = [NOTE] `cargo::non_kebab_case_bin` is set to `warn` in `[lints]`
[HELP] to change the binary name to kebab case, convert `package.name`
  --> Cargo.toml:3:8
   |
 3 - name = "foo_bar"
 3 + name = "foo-bar"
   |
[HELP] to change the binary name to kebab case, specify `bin.name`
  --> Cargo.toml:9:29
   |
 9 ~ non_kebab_case_bin = "warn"
10 + [[bin]]
11 + name = "foo-bar"
12 + path = "src/main.rs"
   |
[CHECKING] foo_bar v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bin_name_from_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
non_kebab_case_bin = "warn"
"#,
        )
        .file("src/bin/foo_bar.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] binaries should have a kebab-case name
  |
1 | [ROOT]/foo/target/.../foo_bar[EXE]
  |                   [..]^^^^^^^
  |
  = [NOTE] `cargo::non_kebab_case_bin` is set to `warn` in `[lints]`
[HELP] to change the binary name to kebab case, convert the file stem
  |
1 - src/bin/foo_bar.rs
1 + src/bin/foo-bar.rs
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn bin_name_from_script_name() {
    let p = cargo_test_support::project()
        .file(
            "foo_bar",
            r#"
---
[lints.cargo]
non_kebab_case_bin = "warn"
---
fn main() {}"#,
        )
        .build();

    p.cargo("check -Zcargo-lints -Zscript --manifest-path foo_bar")
        .masquerade_as_nightly_cargo(&["cargo-lints", "script"])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `[..]`
[WARNING] binaries should have a kebab-case name
  |
1 | [ROOT]/home/.cargo/build/[HASH]/target/.../foo_bar[EXE]
  |                                        [..]^^^^^^^
  |
  = [NOTE] `cargo::non_kebab_case_bin` is set to `warn` in `[lints]`
[HELP] to change the binary name to kebab case, convert the file stem
  |
1 - foo_bar
1 + foo-bar
  |
[CHECKING] foo_bar v0.0.0 ([ROOT]/foo/foo_bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
