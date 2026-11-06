use cargo_platform::{Cfg, CfgExpr, Ident, Platform};
use serde_test::{Token, assert_de_tokens_error, assert_tokens};
use std::fmt;
use std::str::FromStr;

macro_rules! c {
    ($a:ident) => {
        Cfg::Name(Ident {
            name: stringify!($a).to_string(),
            raw: false,
        })
    };
    (r # $a:ident) => {
        Cfg::Name(Ident {
            name: stringify!($a).to_string(),
            raw: true,
        })
    };
    ($a:ident = $e:expr) => {
        Cfg::KeyPair(
            Ident {
                name: stringify!($a).to_string(),
                raw: false,
            },
            $e.to_string(),
        )
    };
    (r # $a:ident = $e:expr) => {
        Cfg::KeyPair(
            Ident {
                name: stringify!($a).to_string(),
                raw: true,
            },
            $e.to_string(),
        )
    };
}

macro_rules! e {
    (any($($t:tt),*)) => (CfgExpr::Any(vec![$(e!($t)),*]));
    (all($($t:tt),*)) => (CfgExpr::All(vec![$(e!($t)),*]));
    (not($($t:tt)*)) => (CfgExpr::Not(Box::new(e!($($t)*))));
    (true) => (CfgExpr::True);
    (false) => (CfgExpr::False);
    (($($t:tt)*)) => (e!($($t)*));
    ($($t:tt)*) => (CfgExpr::Value(c!($($t)*)));
}

#[track_caller]
fn good_canon<T>(s: &'static str, canon: &'static str, expected: T)
where
    T: FromStr
        + PartialEq
        + fmt::Debug
        + serde_core::Serialize
        + for<'de> serde_core::Deserialize<'de>,
    T::Err: fmt::Display,
{
    let c = match T::from_str(s) {
        Ok(c) => c,
        Err(e) => panic!("failed to parse `{}`: {}", s, e),
    };
    assert_eq!(c, expected);

    // Validate the serialization (this tests both directions).
    assert_tokens(&c, &[Token::Str(canon)]);
}

#[track_caller]
fn good<T>(s: &'static str, expected: T)
where
    T: FromStr
        + PartialEq
        + fmt::Debug
        + serde_core::Serialize
        + for<'de> serde_core::Deserialize<'de>,
    T::Err: fmt::Display,
{
    good_canon(s, s, expected);
}

#[track_caller]
fn bad<T>(s: &'static str, err: &'static str)
where
    T: FromStr + fmt::Display + serde_core::Serialize + for<'de> serde_core::Deserialize<'de>,
    T::Err: fmt::Display,
{
    let e = match T::from_str(s) {
        Ok(cfg) => panic!("expected `{}` to not parse but got {}", s, cfg),
        Err(e) => e.to_string(),
    };
    assert_eq!(&e, err);

    assert_de_tokens_error::<T>(&[Token::Str(s)], err);
}

#[test]
fn cfg_syntax() {
    good("foo", c!(foo));
    good("_bar", c!(_bar));
    good_canon(" foo", "foo", c!(foo));
    good_canon(" foo  ", "foo", c!(foo));
    good("r#foo", c!(r # foo));
    good_canon(" foo  = \"bar\"", "foo = \"bar\"", c!(foo = "bar"));
    good_canon("foo=\"\"", "foo = \"\"", c!(foo = ""));
    good_canon("r#foo=\"\"", "r#foo = \"\"", c!(r # foo = ""));
    good_canon(" foo=\"3\"      ", "foo = \"3\"", c!(foo = "3"));
    good("foo = \"3 e\"", c!(foo = "3 e"));
    good_canon(" r#foo = \"3 e\"", "r#foo = \"3 e\"", c!(r # foo = "3 e"));
}

#[test]
fn cfg_syntax_bad() {
    bad::<Cfg>(
        "",
        "failed to parse `` as a cfg expression: expected identifier, but cfg expression ended",
    );
    bad::<Cfg>(
        " ",
        "failed to parse ` ` as a cfg expression: expected identifier, but cfg expression ended",
    );
    bad::<Cfg>(
        "\t",
        "failed to parse `\t` as a cfg expression: unexpected character `\t` in cfg, expected parens, a comma, an identifier, or a string",
    );
    bad::<Cfg>(
        "7",
        "failed to parse `7` as a cfg expression: unexpected character `7` in cfg, expected parens, a comma, an identifier, or a string",
    );
    bad::<Cfg>(
        "=",
        "failed to parse `=` as a cfg expression: expected identifier, found `=`",
    );
    bad::<Cfg>(
        ",",
        "failed to parse `,` as a cfg expression: expected identifier, found `,`",
    );
    bad::<Cfg>(
        "(",
        "failed to parse `(` as a cfg expression: expected identifier, found `(`",
    );
    bad::<Cfg>(
        "foo (",
        "failed to parse `foo (` as a cfg expression: unexpected content `(` found after cfg expression",
    );
    bad::<Cfg>(
        "bar =",
        "failed to parse `bar =` as a cfg expression: expected a string, but cfg expression ended",
    );
    bad::<Cfg>(
        "bar = \"",
        "failed to parse `bar = \"` as a cfg expression: unterminated string in cfg",
    );
    bad::<Cfg>(
        "foo, bar",
        "failed to parse `foo, bar` as a cfg expression: unexpected content `, bar` found after cfg expression",
    );
    bad::<Cfg>(
        "r# foo",
        "failed to parse `r# foo` as a cfg expression: unexpected character ` ` in cfg, expected parens, a comma, an identifier, or a string",
    );
    bad::<Cfg>(
        "r #foo",
        "failed to parse `r #foo` as a cfg expression: unexpected content `#foo` found after cfg expression",
    );
    bad::<Cfg>(
        "r#\"foo\"",
        "failed to parse `r#\"foo\"` as a cfg expression: unexpected character `\"` in cfg, expected parens, a comma, an identifier, or a string",
    );
    bad::<Cfg>(
        "foo = r#\"\"",
        "failed to parse `foo = r#\"\"` as a cfg expression: unexpected character `\"` in cfg, expected parens, a comma, an identifier, or a string",
    );
}

#[test]
fn cfg_expr() {
    good("foo", e!(foo));
    good("_bar", e!(_bar));
    good_canon(" foo", "foo", e!(foo));
    good_canon(" foo  ", "foo", e!(foo));
    good_canon(" foo  = \"bar\"", "foo = \"bar\"", e!(foo = "bar"));
    good_canon("foo=\"\"", "foo = \"\"", e!(foo = ""));
    good_canon(" foo=\"3\"      ", "foo = \"3\"", e!(foo = "3"));
    good("foo = \"3 e\"", e!(foo = "3 e"));

    good("true", e!(true));
    good("false", e!(false));

    good("all()", e!(all()));
    good("all(a)", e!(all(a)));
    good("all(a, b)", e!(all(a, b)));
    good_canon("all(a, )", "all(a)", e!(all(a)));
    good("not(a = \"b\")", e!(not(a = "b")));
    good("not(all(a))", e!(not(all(a))));
}

#[test]
fn cfg_expr_bad() {
    bad::<CfgExpr>(
        " ",
        "failed to parse ` ` as a cfg expression: expected start of a cfg expression, but cfg expression ended",
    );
    bad::<CfgExpr>(
        " all",
        "failed to parse ` all` as a cfg expression: expected `(`, but cfg expression ended",
    );
    bad::<CfgExpr>(
        "all(a",
        "failed to parse `all(a` as a cfg expression: expected `)`, but cfg expression ended",
    );
    bad::<CfgExpr>(
        "not",
        "failed to parse `not` as a cfg expression: expected `(`, but cfg expression ended",
    );
    bad::<CfgExpr>(
        "not(a",
        "failed to parse `not(a` as a cfg expression: expected `)`, but cfg expression ended",
    );
    bad::<CfgExpr>(
        "a = ",
        "failed to parse `a = ` as a cfg expression: expected a string, but cfg expression ended",
    );
    bad::<CfgExpr>(
        "all(not())",
        "failed to parse `all(not())` as a cfg expression: expected identifier, found `)`",
    );
    bad::<CfgExpr>(
        "foo(a)",
        "failed to parse `foo(a)` as a cfg expression: unexpected content `(a)` found after cfg expression",
    );
    bad::<CfgExpr>(
        "cfg(unix)",
        "failed to parse `cfg(unix)` as a cfg expression: unexpected content `(unix)` found after cfg expression",
    );
}

#[test]
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
    assert!(e!(foo).matches(&[c!(r # foo)]));
    assert!(e!(r # foo).matches(&[c!(foo)]));
    assert!(e!(r # foo).matches(&[c!(r # foo)]));

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

#[test]
fn bad_target_name() {
    bad::<Platform>(
        "any(cfg(unix), cfg(windows))",
        "failed to parse `any(cfg(unix), cfg(windows))` as a cfg expression: \
         invalid target specifier: unexpected `(` character, \
         cfg expressions must start with `cfg(`",
    );
    bad::<Platform>(
        "!foo",
        "failed to parse `!foo` as a cfg expression: \
         invalid target specifier: unexpected character ! in target name",
    );
}

#[test]
fn round_trip_platform() {
    fn rt(s: &str) {
        let p = Platform::from_str(s).unwrap();
        let s2 = p.to_string();
        let p2 = Platform::from_str(&s2).unwrap();
        assert_eq!(p, p2);
    }
    rt("x86_64-apple-darwin");
    rt("foo");
    rt("cfg(windows)");
    rt("cfg(target_os = \"windows\")");
    rt(
        "cfg(any(all(any(target_os = \"android\", target_os = \"linux\"), \
         any(target_arch = \"aarch64\", target_arch = \"arm\", target_arch = \"powerpc64\", \
         target_arch = \"x86\", target_arch = \"x86_64\")), \
         all(target_os = \"freebsd\", target_arch = \"x86_64\")))",
    );
}

#[test]
fn check_cfg_attributes() {
    fn ok(s: &str) {
        let p = Platform::Cfg(s.parse().unwrap());
        let mut warnings = Vec::new();
        p.check_cfg_attributes(&mut warnings);
        assert!(
            warnings.is_empty(),
            "Expected no warnings but got: {:?}",
            warnings,
        );
    }

    fn warn(s: &str, names: &[&str]) {
        let p = Platform::Cfg(s.parse().unwrap());
        let mut warnings = Vec::new();
        p.check_cfg_attributes(&mut warnings);
        assert_eq!(
            warnings.len(),
            names.len(),
            "Expecter warnings about {:?} but got {:?}",
            names,
            warnings,
        );
        for (name, warning) in names.iter().zip(warnings.iter()) {
            assert!(
                warning.contains(name),
                "Expected warning about '{}' but got: {}",
                name,
                warning,
            );
        }
    }

    ok("unix");
    ok("windows");
    ok("any(not(unix), windows)");
    ok("foo");
    ok("true");
    ok("false");

    ok("target_arch = \"abc\"");
    ok("target_feature = \"abc\"");
    ok("target_os = \"abc\"");
    ok("target_family = \"abc\"");
    ok("target_env = \"abc\"");
    ok("target_endian = \"abc\"");
    ok("target_pointer_width = \"abc\"");
    ok("target_vendor = \"abc\"");
    ok("bar = \"def\"");

    warn("test", &["test"]);
    warn("debug_assertions", &["debug_assertions"]);
    warn("proc_macro", &["proc_macro"]);
    warn("feature = \"abc\"", &["feature"]);

    warn("any(not(debug_assertions), windows)", &["debug_assertions"]);
    warn(
        "any(not(feature = \"def\"), target_arch = \"abc\")",
        &["feature"],
    );
    warn(
        "any(not(target_os = \"windows\"), proc_macro)",
        &["proc_macro"],
    );
    warn(
        "any(not(feature = \"windows\"), proc_macro)",
        &["feature", "proc_macro"],
    );
    warn(
        "all(not(debug_assertions), any(windows, proc_macro))",
        &["debug_assertions", "proc_macro"],
    );
}
