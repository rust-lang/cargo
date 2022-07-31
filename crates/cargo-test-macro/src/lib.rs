extern crate proc_macro;

use proc_macro::*;
use std::process::Command;
use std::sync::Once;

#[proc_macro_attribute]
pub fn cargo_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Ideally these options would be embedded in the test itself. However, I
    // find it very helpful to have the test clearly state whether or not it
    // is ignored. It would be nice to have some kind of runtime ignore
    // support (such as
    // https://internals.rust-lang.org/t/pre-rfc-skippable-tests/14611).
    //
    // Unfortunately a big drawback here is that if the environment changes
    // (such as the existence of the `git` CLI), this will not trigger a
    // rebuild and the test will still be ignored. In theory, something like
    // `tracked_env` or `tracked_path`
    // (https://github.com/rust-lang/rust/issues/99515) could help with this,
    // but they don't really handle the absence of files well.
    let mut ignore = false;
    let mut requires_reason = false;
    let mut found_reason = false;
    let is_not_nightly = !version().1;
    for rule in split_rules(attr) {
        match rule.as_str() {
            "build_std_real" => {
                // Only run the "real" build-std tests on nightly and with an
                // explicit opt-in (these generally only work on linux, and
                // have some extra requirements, and are slow, and can pollute
                // the environment since it downloads dependencies).
                ignore |= is_not_nightly;
                ignore |= option_env!("CARGO_RUN_BUILD_STD_TESTS").is_none();
            }
            "build_std_mock" => {
                // Only run the "mock" build-std tests on nightly and disable
                // for windows-gnu which is missing object files (see
                // https://github.com/rust-lang/wg-cargo-std-aware/issues/46).
                ignore |= is_not_nightly;
                ignore |= cfg!(all(target_os = "windows", target_env = "gnu"));
            }
            "nightly" => {
                requires_reason = true;
                ignore |= is_not_nightly;
            }
            s if s.starts_with("requires_") => {
                let command = &s[9..];
                ignore |= !has_command(command);
            }
            s if s.starts_with(">=1.") => {
                requires_reason = true;
                let min_minor = s[4..].parse().unwrap();
                ignore |= version().0 < min_minor;
            }
            s if s.starts_with("reason=") => {
                found_reason = true;
            }
            _ => panic!("unknown rule {:?}", rule),
        }
    }
    if requires_reason && !found_reason {
        panic!(
            "#[cargo_test] with a rule also requires a reason, \
            such as #[cargo_test(nightly, reason = \"needs -Z unstable-thing\")]"
        );
    }

    let span = Span::call_site();
    let mut ret = TokenStream::new();
    let add_attr = |ret: &mut TokenStream, attr_name| {
        ret.extend(Some(TokenTree::from(Punct::new('#', Spacing::Alone))));
        let attr = TokenTree::from(Ident::new(attr_name, span));
        ret.extend(Some(TokenTree::from(Group::new(
            Delimiter::Bracket,
            attr.into(),
        ))));
    };
    add_attr(&mut ret, "test");
    if ignore {
        add_attr(&mut ret, "ignore");
    }

    for token in item {
        let group = match token {
            TokenTree::Group(g) => {
                if g.delimiter() == Delimiter::Brace {
                    g
                } else {
                    ret.extend(Some(TokenTree::Group(g)));
                    continue;
                }
            }
            other => {
                ret.extend(Some(other));
                continue;
            }
        };

        let mut new_body = to_token_stream(
            r#"let _test_guard = {
                let tmp_dir = option_env!("CARGO_TARGET_TMPDIR");
                cargo_test_support::paths::init_root(tmp_dir)
            };"#,
        );

        new_body.extend(group.stream());
        ret.extend(Some(TokenTree::from(Group::new(
            group.delimiter(),
            new_body,
        ))));
    }

    ret
}

fn split_rules(t: TokenStream) -> Vec<String> {
    let tts: Vec<_> = t.into_iter().collect();
    tts.split(|tt| match tt {
        TokenTree::Punct(p) => p.as_char() == ',',
        _ => false,
    })
    .filter(|parts| !parts.is_empty())
    .map(|parts| {
        parts
            .into_iter()
            .map(|part| part.to_string())
            .collect::<String>()
    })
    .collect()
}

fn to_token_stream(code: &str) -> TokenStream {
    code.parse().unwrap()
}

static mut VERSION: (u32, bool) = (0, false);

fn version() -> &'static (u32, bool) {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let output = Command::new("rustc")
            .arg("-V")
            .output()
            .expect("rustc should run");
        let stdout = std::str::from_utf8(&output.stdout).expect("utf8");
        let vers = stdout.split_whitespace().skip(1).next().unwrap();
        let is_nightly = option_env!("CARGO_TEST_DISABLE_NIGHTLY").is_none()
            && (vers.contains("-nightly") || vers.contains("-dev"));
        let minor = vers.split('.').skip(1).next().unwrap().parse().unwrap();
        unsafe { VERSION = (minor, is_nightly) }
    });
    unsafe { &VERSION }
}

fn has_command(command: &str) -> bool {
    let output = match Command::new(command).arg("--version").output() {
        Ok(output) => output,
        Err(e) => {
            // hg is not installed on GitHub macos.
            // Consider installing it if Cargo gains more hg support, but
            // otherwise it isn't critical.
            if is_ci() && !(cfg!(target_os = "macos") && command == "hg") {
                panic!(
                    "expected command `{}` to be somewhere in PATH: {}",
                    command, e
                );
            }
            return false;
        }
    };
    if !output.status.success() {
        panic!(
            "expected command `{}` to be runnable, got error {}:\n\
            stderr:{}\n\
            stdout:{}\n",
            command,
            output.status,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }
    true
}

/// Whether or not this running in a Continuous Integration environment.
fn is_ci() -> bool {
    // Consider using `tracked_env` instead of option_env! when it is stabilized.
    // `tracked_env` will handle changes, but not require rebuilding the macro
    // itself like option_env does.
    option_env!("CI").is_some() || option_env!("TF_BUILD").is_some()
}
