extern crate cargotest;
extern crate hamcrest;

use cargotest::support::git;
use cargotest::support::paths;
use cargotest::support::registry::{registry, Package};
use cargotest::support::{execs, project};
use hamcrest::assert_that;

#[test]
fn override_simple() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        ");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[UPDATING] git repository `[..]`
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] local v0.0.1 (file://[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn missing_version() {
    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            foo = { git = 'https://example.com' }
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  replacements must specify a version to replace, but `foo` does not
"));
}

#[test]
fn different_version() {
    Package::new("foo", "0.2.0").publish();
    Package::new("foo", "0.1.0").publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = "0.2.0"
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  replacements cannot specify a version requirement, but found one for [..]
"));
}

#[test]
fn transitive() {
    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.2.0")
            .dep("foo", "0.1.0")
            .file("src/lib.rs", "extern crate foo; fn bar() { foo::foo(); }")
            .publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.2.0"

            [replace]
            "foo:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[UPDATING] git repository `[..]`
[DOWNLOADING] bar v0.2.0 (registry [..])
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] bar v0.2.0 (registry [..])
[COMPILING] local v0.0.1 (file://[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
}

#[test]
fn persists_across_rebuilds() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        ");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[UPDATING] git repository `file://[..]`
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] local v0.0.1 (file://[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn replace_registry_with_path() {
    Package::new("foo", "0.1.0").publish();

    project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = { path = "../foo" }
        "#)
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        ");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] local v0.0.1 (file://[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn use_a_spec_to_select() {
    Package::new("foo", "0.1.1")
            .file("src/lib.rs", "pub fn foo1() {}")
            .publish();
    Package::new("foo", "0.2.0").publish();
    Package::new("bar", "0.1.1")
            .dep("foo", "0.2")
            .file("src/lib.rs", "
                extern crate foo;
                pub fn bar() { foo::foo3(); }
            ")
            .publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo3() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"
            foo = "0.1"

            [replace]
            "foo:0.2.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "
            extern crate foo;
            extern crate bar;

            pub fn local() {
                foo::foo1();
                bar::bar();
            }
        ");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[UPDATING] git repository `[..]`
[DOWNLOADING] [..]
[DOWNLOADING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] local v0.0.1 (file://[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn override_adds_some_deps() {
    Package::new("foo", "0.1.1").publish();
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = "0.1"
        "#)
        .file("src/lib.rs", "");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"

            [replace]
            "bar:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[UPDATING] git repository `[..]`
[DOWNLOADING] foo v0.1.1 (registry [..])
[COMPILING] foo v0.1.1 (registry [..])
[COMPILING] bar v0.1.0 ([..])
[COMPILING] local v0.0.1 (file://[..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));

    Package::new("foo", "0.1.2").publish();
    assert_that(p.cargo("update").arg("-p").arg(&format!("{}#bar", foo.url())),
                execs().with_status(0).with_stderr("\
[UPDATING] git repository `file://[..]`
"));
    assert_that(p.cargo("update").arg("-p").arg(&format!("{}#bar", registry())),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
"));

    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
}

#[test]
fn locked_means_locked_yes_no_seriously_i_mean_locked() {
    // this in theory exercises #2041
    Package::new("foo", "0.1.0").publish();
    Package::new("foo", "0.2.0").publish();
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = "*"
        "#)
        .file("src/lib.rs", "");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1"
            bar = "0.1"

            [replace]
            "bar:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
}

#[test]
fn override_wrong_name() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1"

            [replace]
            "foo:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[UPDATING] git repository [..]
error: no matching package for override `foo:0.1.0` found
location searched: file://[..]
version required: = 0.1.0
"));
}

#[test]
fn override_with_nothing() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("src/lib.rs", "");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1"

            [replace]
            "foo:0.1.0" = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[UPDATING] git repository [..]
[ERROR] failed to load source for a dependency on `foo`

Caused by:
  Unable to update file://[..]

Caused by:
  Could not find Cargo.toml in `[..]`
"));
}

#[test]
fn override_wrong_version() {
    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [replace]
            "foo:0.1.0" = { git = 'https://example.com', version = '0.2.0' }
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  replacements cannot specify a version requirement, but found one for `foo:0.1.0`
"));
}

#[test]
fn multiple_specs() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = {{ git = '{0}' }}
            "{1}#foo:0.1.0" = {{ git = '{0}' }}
        "#, foo.url(), registry()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[UPDATING] git repository [..]
error: overlapping replacement specifications found:

  * [..]
  * [..]

both specifications match: foo v0.1.0 ([..])
"));
}

#[test]
fn test_override_dep() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = {{ git = '{0}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("test").arg("-p").arg("foo"),
                execs().with_status(101)
                       .with_stderr_contains("\
error: There are multiple `foo` packages in your project, and the [..]
Please re-run this command with [..]
  [..]#foo:0.1.0
  [..]#foo:0.1.0
"));
}

#[test]
fn update() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [replace]
            "foo:0.1.0" = {{ git = '{0}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("generate-lockfile"),
                execs().with_status(0));
    assert_that(p.cargo("update"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`
[UPDATING] git repository `[..]`
"));
}

// local -> near -> far
// near is overridden with itself
#[test]
fn no_override_self() {
    let deps = git::repo(&paths::root().join("override"))

        .file("far/Cargo.toml", r#"
            [package]
            name = "far"
            version = "0.1.0"
            authors = []
        "#)
        .file("far/src/lib.rs", "")

        .file("near/Cargo.toml", r#"
            [package]
            name = "near"
            version = "0.1.0"
            authors = []

            [dependencies]
            far = { path = "../far" }
        "#)
        .file("near/src/lib.rs", r#"
            #![no_std]
            pub extern crate far;
        "#);

    deps.build();

    let p = project("local")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            near = {{ git = '{0}' }}

            [replace]
            "near:0.1.0" = {{ git = '{0}' }}
        "#, deps.url()))
        .file("src/lib.rs", r#"
            #![no_std]
            pub extern crate near;
        "#);

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(0));
}
