use std::fmt;
use std::str::FromStr;

use crate::support::registry::Package;
use crate::support::rustc_host;
use crate::support::{basic_manifest, project};
use cargo::util::{Cfg, CfgExpr};

macro_rules! c {
    ($a:ident) => {
        Cfg::Name(stringify!($a).to_string())
    };
    ($a:ident = $e:expr) => {
        Cfg::KeyPair(stringify!($a).to_string(), $e.to_string())
    };
}

macro_rules! e {
    (any($($t:tt),*)) => (CfgExpr::Any(vec![$(e!($t)),*]));
    (all($($t:tt),*)) => (CfgExpr::All(vec![$(e!($t)),*]));
    (not($($t:tt)*)) => (CfgExpr::Not(Box::new(e!($($t)*))));
    (($($t:tt)*)) => (e!($($t)*));
    ($($t:tt)*) => (CfgExpr::Value(c!($($t)*)));
}

fn good<T>(s: &str, expected: T)
where
    T: FromStr + PartialEq + fmt::Debug,
    T::Err: fmt::Display,
{
    let c = match T::from_str(s) {
        Ok(c) => c,
        Err(e) => panic!("failed to parse `{}`: {}", s, e),
    };
    assert_eq!(c, expected);
}

fn bad<T>(s: &str, err: &str)
where
    T: FromStr + fmt::Display,
    T::Err: fmt::Display,
{
    let e = match T::from_str(s) {
        Ok(cfg) => panic!("expected `{}` to not parse but got {}", s, cfg),
        Err(e) => e.to_string(),
    };
    assert!(
        e.contains(err),
        "when parsing `{}`,\n\"{}\" not contained \
         inside: {}",
        s,
        err,
        e
    );
}

#[cargo_test]
fn cfg_syntax() {
    good("foo", c!(foo));
    good("_bar", c!(_bar));
    good(" foo", c!(foo));
    good(" foo  ", c!(foo));
    good(" foo  = \"bar\"", c!(foo = "bar"));
    good("foo=\"\"", c!(foo = ""));
    good(" foo=\"3\"      ", c!(foo = "3"));
    good("foo = \"3 e\"", c!(foo = "3 e"));
}

#[cargo_test]
fn cfg_syntax_bad() {
    bad::<Cfg>("", "found nothing");
    bad::<Cfg>(" ", "found nothing");
    bad::<Cfg>("\t", "unexpected character");
    bad::<Cfg>("7", "unexpected character");
    bad::<Cfg>("=", "expected identifier");
    bad::<Cfg>(",", "expected identifier");
    bad::<Cfg>("(", "expected identifier");
    bad::<Cfg>("foo (", "malformed cfg value");
    bad::<Cfg>("bar =", "expected a string");
    bad::<Cfg>("bar = \"", "unterminated string");
    bad::<Cfg>("foo, bar", "malformed cfg value");
}

#[cargo_test]
fn cfg_expr() {
    good("foo", e!(foo));
    good("_bar", e!(_bar));
    good(" foo", e!(foo));
    good(" foo  ", e!(foo));
    good(" foo  = \"bar\"", e!(foo = "bar"));
    good("foo=\"\"", e!(foo = ""));
    good(" foo=\"3\"      ", e!(foo = "3"));
    good("foo = \"3 e\"", e!(foo = "3 e"));

    good("all()", e!(all()));
    good("all(a)", e!(all(a)));
    good("all(a, b)", e!(all(a, b)));
    good("all(a, )", e!(all(a)));
    good("not(a = \"b\")", e!(not(a = "b")));
    good("not(all(a))", e!(not(all(a))));
}

#[cargo_test]
fn cfg_expr_bad() {
    bad::<CfgExpr>(" ", "found nothing");
    bad::<CfgExpr>(" all", "expected `(`");
    bad::<CfgExpr>("all(a", "expected `)`");
    bad::<CfgExpr>("not", "expected `(`");
    bad::<CfgExpr>("not(a", "expected `)`");
    bad::<CfgExpr>("a = ", "expected a string");
    bad::<CfgExpr>("all(not())", "expected identifier");
    bad::<CfgExpr>("foo(a)", "consider using all() or any() explicitly");
}

#[cargo_test]
fn cfg_matches() {
    assert!(e!(foo).matches(&[c!(bar), c!(foo), c!(baz)]));
    assert!(e!(any(foo)).matches(&[c!(bar), c!(foo), c!(baz)]));
    assert!(e!(any(foo, bar)).matches(&[c!(bar)]));
    assert!(e!(any(foo, bar)).matches(&[c!(foo)]));
    assert!(e!(all(foo, bar)).matches(&[c!(foo), c!(bar)]));
    assert!(e!(all(foo, bar)).matches(&[c!(foo), c!(bar)]));
    assert!(e!(not(foo)).matches(&[c!(bar)]));
    assert!(e!(not(foo)).matches(&[]));
    assert!(e!(any((not(foo)), (all(foo, bar)))).matches(&[c!(bar)]));
    assert!(e!(any((not(foo)), (all(foo, bar)))).matches(&[c!(foo), c!(bar)]));

    assert!(!e!(foo).matches(&[]));
    assert!(!e!(foo).matches(&[c!(bar)]));
    assert!(!e!(foo).matches(&[c!(fo)]));
    assert!(!e!(any(foo)).matches(&[]));
    assert!(!e!(any(foo)).matches(&[c!(bar)]));
    assert!(!e!(any(foo)).matches(&[c!(bar), c!(baz)]));
    assert!(!e!(all(foo)).matches(&[c!(bar), c!(baz)]));
    assert!(!e!(all(foo, bar)).matches(&[c!(bar)]));
    assert!(!e!(all(foo, bar)).matches(&[c!(foo)]));
    assert!(!e!(all(foo, bar)).matches(&[]));
    assert!(!e!(not(bar)).matches(&[c!(bar)]));
    assert!(!e!(not(bar)).matches(&[c!(baz), c!(bar)]));
    assert!(!e!(any((not(foo)), (all(foo, bar)))).matches(&[c!(foo)]));
}

#[cargo_test]
fn cfg_easy() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg(unix)'.dependencies]
            b = { path = 'b' }
            [target."cfg(windows)".dependencies]
            b = { path = 'b' }
        "#,
        )
        .file("src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn dont_include() {
    let other_family = if cfg!(unix) { "windows" } else { "unix" };
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg({})'.dependencies]
            b = {{ path = 'b' }}
        "#,
                other_family
            ),
        )
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn works_through_the_registry() {
    Package::new("baz", "0.1.0").publish();
    Package::new("bar", "0.1.0")
        .target_dep("baz", "0.1.0", "cfg(unix)")
        .target_dep("baz", "0.1.0", "cfg(windows)")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate bar;",
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] [..] index
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
[DOWNLOADED] [..]
[COMPILING] baz v0.1.0
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn ignore_version_from_other_platform() {
    let this_family = if cfg!(unix) { "unix" } else { "windows" };
    let other_family = if cfg!(unix) { "windows" } else { "unix" };
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.'cfg({})'.dependencies]
            bar = "0.1.0"

            [target.'cfg({})'.dependencies]
            bar = "0.2.0"
        "#,
                this_family, other_family
            ),
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate bar;",
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] [..] index
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn bad_target_spec() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.'cfg(4)'.dependencies]
            bar = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  failed to parse `4` as a cfg expression

Caused by:
  unexpected character in cfg `4`, [..]
",
        )
        .run();
}

#[cargo_test]
fn bad_target_spec2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.'cfg(bar =)'.dependencies]
            baz = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  failed to parse `bar =` as a cfg expression

Caused by:
  expected a string, found nothing
",
        )
        .run();
}

#[cargo_test]
fn multiple_match_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg(unix)'.dependencies]
            b = {{ path = 'b' }}
            [target.'cfg(target_family = "unix")'.dependencies]
            b = {{ path = 'b' }}
            [target."cfg(windows)".dependencies]
            b = {{ path = 'b' }}
            [target.'cfg(target_family = "windows")'.dependencies]
            b = {{ path = 'b' }}
            [target."cfg(any(windows, unix))".dependencies]
            b = {{ path = 'b' }}

            [target.{}.dependencies]
            b = {{ path = 'b' }}
        "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn any_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target."cfg(any(windows, unix))".dependencies]
            b = { path = 'b' }
        "#,
        )
        .file("src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("build -v").run();
}

// https://github.com/rust-lang/cargo/issues/5313
#[cargo_test]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
fn cfg_looks_at_rustflags_for_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg(with_b)'.dependencies]
            b = { path = 'b' }
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[cfg(with_b)]
            extern crate b;

            fn main() { b::foo(); }
        "#,
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("build --target x86_64-unknown-linux-gnu")
        .env("RUSTFLAGS", "--cfg with_b")
        .run();
}
