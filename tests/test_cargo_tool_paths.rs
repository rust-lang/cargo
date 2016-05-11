use support::{path2url, project, execs};
use hamcrest::assert_that;

fn setup() {
}

test!(pathless_tools {
    let target = ::rustc_host();

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", &format!(r#"
            [target.{}]
            ar = "nonexistent-ar"
            linker = "nonexistent-linker"
        "#, target));

    assert_that(foo.cargo_process("build").arg("--verbose"),
                execs().with_stdout(&format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc [..] -C ar=nonexistent-ar -C linker=nonexistent-linker [..]`
", url = foo.url())))
});

test!(absolute_tools {
    let target = ::rustc_host();

    // Escaped as they appear within a TOML config file
    let config = if cfg!(windows) {
        (r#"C:\\bogus\\nonexistent-ar"#, r#"C:\\bogus\\nonexistent-linker"#)
    } else {
        (r#"/bogus/nonexistent-ar"#, r#"/bogus/nonexistent-linker"#)
    };

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", &format!(r#"
            [target.{target}]
            ar = "{ar}"
            linker = "{linker}"
        "#, target = target, ar = config.0, linker = config.1));

    let output = if cfg!(windows) {
        (r#"C:\bogus\nonexistent-ar"#, r#"C:\bogus\nonexistent-linker"#)
    } else {
        (r#"/bogus/nonexistent-ar"#, r#"/bogus/nonexistent-linker"#)
    };

    assert_that(foo.cargo_process("build").arg("--verbose"),
                execs().with_stdout(&format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc [..] -C ar={ar} -C linker={linker} [..]`
", url = foo.url(), ar = output.0, linker = output.1)))
});

test!(relative_tools {
    let target = ::rustc_host();

    // Escaped as they appear within a TOML config file
    let config = if cfg!(windows) {
        (r#".\\nonexistent-ar"#, r#".\\tools\\nonexistent-linker"#)
    } else {
        (r#"./nonexistent-ar"#, r#"./tools/nonexistent-linker"#)
    };

    // Funky directory structure to test that relative tool paths are made absolute
    // by reference to the `.cargo/..` directory and not to (for example) the CWD.
    let origin = project("origin")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
        "#)
        .file("foo/src/lib.rs", "")
        .file(".cargo/config", &format!(r#"
            [target.{target}]
            ar = "{ar}"
            linker = "{linker}"
        "#, target = target, ar = config.0, linker = config.1));

    let foo_path = origin.root().join("foo");
    let foo_url = path2url(foo_path.clone());
    let prefix = origin.root().into_os_string().into_string().unwrap();
    let output = if cfg!(windows) {
        (format!(r#"{}\.\nonexistent-ar"#, prefix),
         format!(r#"{}\.\tools\nonexistent-linker"#, prefix))
    } else {
        (format!(r#"{}/./nonexistent-ar"#, prefix),
         format!(r#"{}/./tools/nonexistent-linker"#, prefix))
    };

    assert_that(origin.cargo_process("build").cwd(foo_path).arg("--verbose"),
                execs().with_stdout(&format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc [..] -C ar={ar} -C linker={linker} [..]`
", url = foo_url, ar = output.0, linker = output.1)))
});
