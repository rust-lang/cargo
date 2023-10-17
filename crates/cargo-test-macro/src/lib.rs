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
    let mut explicit_reason = None;
    let mut implicit_reasons = Vec::new();
    macro_rules! set_ignore {
        ($predicate:expr, $($arg:tt)*) => {
            let p = $predicate;
            ignore |= p;
            if p {
                implicit_reasons.push(std::fmt::format(format_args!($($arg)*)));
            }
        };
    }
    let is_not_nightly = !version().1;
    for rule in split_rules(attr) {
        match rule.as_str() {
            "build_std_real" => {
                // Only run the "real" build-std tests on nightly and with an
                // explicit opt-in (these generally only work on linux, and
                // have some extra requirements, and are slow, and can pollute
                // the environment since it downloads dependencies).
                set_ignore!(is_not_nightly, "requires nightly");
                set_ignore!(
                    option_env!("CARGO_RUN_BUILD_STD_TESTS").is_none(),
                    "CARGO_RUN_BUILD_STD_TESTS must be set"
                );
            }
            "build_std_mock" => {
                // Only run the "mock" build-std tests on nightly and disable
                // for windows-gnu which is missing object files (see
                // https://github.com/rust-lang/wg-cargo-std-aware/issues/46).
                set_ignore!(is_not_nightly, "requires nightly");
                set_ignore!(
                    cfg!(all(target_os = "windows", target_env = "gnu")),
                    "does not work on windows-gnu"
                );
            }
            "container_test" => {
                // These tests must be opt-in because they require docker.
                set_ignore!(
                    option_env!("CARGO_CONTAINER_TESTS").is_none(),
                    "CARGO_CONTAINER_TESTS must be set"
                );
            }
            "public_network_test" => {
                // These tests must be opt-in because they touch the public
                // network. The use of these should be **EXTREMELY RARE**, and
                // should only touch things which would nearly certainly work
                // in CI (like github.com).
                set_ignore!(
                    option_env!("CARGO_PUBLIC_NETWORK_TESTS").is_none(),
                    "CARGO_PUBLIC_NETWORK_TESTS must be set"
                );
            }
            "nightly" => {
                requires_reason = true;
                set_ignore!(is_not_nightly, "requires nightly");
            }
            s if s.starts_with("requires_") => {
                let command = &s[9..];
                set_ignore!(!has_command(command), "{command} not installed");
            }
            s if s.starts_with(">=1.") => {
                requires_reason = true;
                let min_minor = s[4..].parse().unwrap();
                let minor = version().0;
                set_ignore!(minor < min_minor, "requires rustc 1.{minor} or newer");
            }
            s if s.starts_with("reason=") => {
                explicit_reason = Some(s[7..].parse().unwrap());
            }
            s if s.starts_with("ignore_windows=") => {
                set_ignore!(cfg!(windows), "{}", &s[16..s.len() - 1]);
            }
            _ => panic!("unknown rule {:?}", rule),
        }
    }
    if requires_reason && explicit_reason.is_none() {
        panic!(
            "#[cargo_test] with a rule also requires a reason, \
            such as #[cargo_test(nightly, reason = \"needs -Z unstable-thing\")]"
        );
    }

    // Construct the appropriate attributes.
    let span = Span::call_site();
    let mut ret = TokenStream::new();
    let add_attr = |ret: &mut TokenStream, attr_name, attr_input| {
        ret.extend(Some(TokenTree::from(Punct::new('#', Spacing::Alone))));
        let attr = TokenTree::from(Ident::new(attr_name, span));
        let mut attr_stream: TokenStream = attr.into();
        if let Some(input) = attr_input {
            attr_stream.extend(input);
        }
        ret.extend(Some(TokenTree::from(Group::new(
            Delimiter::Bracket,
            attr_stream,
        ))));
    };
    add_attr(&mut ret, "test", None);
    if ignore {
        let reason = explicit_reason
            .or_else(|| {
                (!implicit_reasons.is_empty())
                    .then(|| TokenTree::from(Literal::string(&implicit_reasons.join(", "))).into())
            })
            .map(|reason: TokenStream| {
                let mut stream = TokenStream::new();
                stream.extend(Some(TokenTree::from(Punct::new('=', Spacing::Alone))));
                stream.extend(Some(reason));
                stream
            });
        add_attr(&mut ret, "ignore", reason);
    }

    // Find where the function body starts, and add the boilerplate at the start.
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
            // hg is not installed on GitHub macOS or certain constrained
            // environments like Docker. Consider installing it if Cargo gains
            // more hg support, but otherwise it isn't critical.
            if is_ci() && command != "hg" {
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
