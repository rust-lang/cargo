use crate::support::{basic_lib_manifest, project};

#[test]
fn build_profile_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [profile.build]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `build-profile` is required

consider adding `cargo-features = [\"build-profile\"]` to the manifest
",
        )
        .run();
}

#[test]
fn build_profile_basic() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile"]

            [package]
            name = "foo"
            version = "0.0.1"

            [profile.build]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name build_script_build",
                "-C opt-level=3",
            ],
            &["-C debuginfo", "-C incremental"],
        )
        .run();
}

#[test]
fn build_profile_default_disabled() {
    // Verify the defaults are not affected if build-profile is not enabled.
    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name build_script_build",
                "-C debuginfo=2",
            ],
            &["-C opt-level"],
        )
        .run();

    p.cargo("build -v --release")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name build_script_build",
                "-C opt-level=3",
            ],
            &["-C debuginfo"],
        )
        .run();
}

#[test]
fn build_profile_default_enabled() {
    // Check the change in defaults when the feature is enabled.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile"]

            [package]
            name = "foo"
            version = "0.0.1"
        "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name build_script_build"],
            &["-C debuginfo", "-C opt-level"],
        )
        .run();

    p.cargo("build -v --release")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name build_script_build"],
            &["-C debuginfo", "-C opt-level"],
        )
        .run();
}

#[test]
fn build_profile_with_override() {
    // Check with `overrides` in `profile.build`.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile", "profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [profile.build.overrides.bar]
            opt-level = 3

            [dependencies]
            bar = { path = "bar" }

            [build-dependencies]
            bar = { path = "bar" }
        "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("build.rs", "extern crate bar; fn main() {}")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        // bar as a build-dependency
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name bar", "-C opt-level=3"],
            &["-C debuginfo"],
        )
        // bar as a regular dependency
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name bar", "-C debuginfo=2"],
            &["-C opt-level"],
        )
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name build_script_build"],
            &["-C opt-level", "-C debuginfo"],
        )
        .run();
}

#[test]
fn build_profile_rejects_build_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile", "profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [profile.build.build-override]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  `build` profile cannot specify build overrides.
",
        )
        .run();
}

#[test]
fn build_profile_with_other_build_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile", "profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [profile.build]
            codegen-units = 1

            [profile.dev]
            codegen-units = 2

            [profile.dev.build-override]
            codegen-units = 3

            [profile.release]
            codegen-units = 4
        "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    // build script = 3
    // lib = 2
    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        // Note: it inherits all settings from profile.dev (including debuginfo)
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name build_script_build",
                "-C codegen-units=3",
                "-C debuginfo=2",
            ],
            &["-C opt-level"],
        )
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name foo",
                "-C codegen-units=2",
                "-C debuginfo=2",
            ],
            &[],
        )
        .run();

    // build script = 1
    // lib = 4
    p.cargo("build -v --release")
        .masquerade_as_nightly_cargo()
        // This is not overridden.
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name build_script_build",
                "-C codegen-units=1",
            ],
            &["-C opt-level", "-C debuginfo"],
        )
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name foo", "-C codegen-units=4"],
            &["-C debuginfo"],
        )
        .run();
}

#[test]
fn build_profile_default_with_pm_build_override() {
    // Check that enabling build-profile engages the ability for
    // build-overrides to affect proc-macros.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile", "profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [profile.dev.build-override]
            codegen-units = 3

            [dependencies]
            pm = { path = "pm" }
        "#,
        )
        .file("src/lib.rs", "extern crate pm;")
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"
            [lib]
            proc-macro = true
        "#,
        )
        .file("pm/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name pm",
                "-C codegen-units=3",
                "-C debuginfo=2",
            ],
            &["-C opt-level"],
        )
        .run();
}

#[test]
fn build_profile_config() {
    // Check `[profile.build]` inside .cargo/config.
    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [profile.build]
            codegen-units = 3
        "#,
        )
        .build();

    p.cargo("build -v -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("\
[WARNING] profile `build` in config file will be ignored for manifest `[CWD]/Cargo.toml` \
because \"build-profile\" feature is not enabled")
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name build_script_build", "-C debuginfo=2"],
            &["-C codegen-units", "-C opt-level"])
        .run();

    p.change_file(
        "Cargo.toml",
        r#"
        cargo-features = ["build-profile"]

        [package]
        name = "foo"
        version = "0.0.1"
    "#,
    );

    p.cargo("build -v -Z config-profile")
        .masquerade_as_nightly_cargo()
        .with_stderr_line_without(
            &[
                "[RUNNING] `rustc --crate-name build_script_build",
                "-C codegen-units=3",
            ],
            &["-C debuginfo=2", "-C opt-level"],
        )
        .run();
}

#[test]
fn proc_macro_default_disabled() {
    // Make sure proc-macros are not affected if feature is not enabled.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [dependencies]
            pm = { path = "pm" }
        "#,
        )
        .file("src/lib.rs", "extern crate pm;")
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"
            [lib]
            proc-macro = true
        "#,
        )
        .file("pm/src/lib.rs", "")
        .build();

    p.cargo("build -v --release")
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name pm", "-C opt-level=3"],
            &["-C debuginfo=2"],
        )
        .run();
}

#[test]
fn proc_macro_default_enabled() {
    // Check that proc-macros *are* affected when build-profile is enabled,
    // but no profile settings are set (checking the default settings).
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["build-profile"]
            [package]
            name = "foo"
            version = "0.1.0"
            [dependencies]
            bar = { path = "bar" }
            pm = { path = "pm" }
        "#,
        )
        .file(
            "src/lib.rs",
            "\
            extern crate pm;
            extern crate bar;
        ",
        )
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"
            [lib]
            proc-macro = true
            [dependencies]
            bar = { path = "../bar" }
        "#,
        )
        .file("pm/src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -v --release")
        .masquerade_as_nightly_cargo()
        // bar for the proc-macro
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name bar"],
            &["-C opt-level", "-C debuginfo"],
        )
        // bar as a normal dependency
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name bar", "-C opt-level=3"],
            &["-C debuginfo"],
        )
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name pm"],
            &["-C debuginfo", "-C opt-level"],
        )
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name foo", "-C opt-level=3"],
            &["-C debuginfo"],
        )
        .run();
}
