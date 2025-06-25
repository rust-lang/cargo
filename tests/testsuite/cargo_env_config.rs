//! Tests for `[env]` config.

use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::str;
use cargo_test_support::{basic_bin_manifest, project};

#[cargo_test]
fn env_basic() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        fn main() {
            println!( "compile-time:{}", env!("ENV_TEST_1233") );
            println!( "run-time:{}", env::var("ENV_TEST_1233").unwrap());
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST_1233 = "Hello"
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
compile-time:Hello
run-time:Hello

"#]])
        .run();
}

#[cargo_test]
fn env_invalid() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
        fn main() {
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST_BOOL = false
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `env.ENV_TEST_BOOL`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: could not load config key `env.ENV_TEST_BOOL`

Caused by:
  invalid type: boolean `false`, expected a string or map

"#]])
        .run();
}

#[cargo_test]
fn env_no_disallowed() {
    // Checks for keys that are not allowed in the [env] table.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    for disallowed in &["CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"] {
        p.change_file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [env]
                    {disallowed} = "foo"
                "#
            ),
        );
        p.cargo("check")
            .with_status(101)
            .with_stderr_data(format!(
                "\
[ERROR] setting the `{disallowed}` environment variable \
is not supported in the `[env]` configuration table
"
            ))
            .run();
    }
}

#[cargo_test]
fn env_force() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        fn main() {
            println!( "ENV_TEST_FORCED:{}", env!("ENV_TEST_FORCED") );
            println!( "ENV_TEST_UNFORCED:{}", env!("ENV_TEST_UNFORCED") );
            println!( "ENV_TEST_UNFORCED_DEFAULT:{}", env!("ENV_TEST_UNFORCED_DEFAULT") );
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST_UNFORCED_DEFAULT = "from-config"
                ENV_TEST_UNFORCED = { value = "from-config", force = false }
                ENV_TEST_FORCED = { value = "from-config", force = true }
            "#,
        )
        .build();

    p.cargo("run")
        .env("ENV_TEST_FORCED", "from-env")
        .env("ENV_TEST_UNFORCED", "from-env")
        .env("ENV_TEST_UNFORCED_DEFAULT", "from-env")
        .with_stdout_data(str![[r#"
ENV_TEST_FORCED:from-config
ENV_TEST_UNFORCED:from-env
ENV_TEST_UNFORCED_DEFAULT:from-env

"#]])
        .run();
}

#[cargo_test]
fn env_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo2"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        use std::path::Path;
        fn main() {
            println!( "ENV_TEST_REGULAR:{}", env!("ENV_TEST_REGULAR") );
            println!( "ENV_TEST_REGULAR_DEFAULT:{}", env!("ENV_TEST_REGULAR_DEFAULT") );
            println!( "ENV_TEST_RELATIVE:{}", env!("ENV_TEST_RELATIVE") );

            assert!( Path::new(env!("ENV_TEST_RELATIVE")).is_absolute() );
            assert!( !Path::new(env!("ENV_TEST_REGULAR")).is_absolute() );
            assert!( !Path::new(env!("ENV_TEST_REGULAR_DEFAULT")).is_absolute() );
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST_REGULAR = { value = "Cargo.toml", relative = false }
                ENV_TEST_REGULAR_DEFAULT = "Cargo.toml"
                ENV_TEST_RELATIVE = { value = "Cargo.toml", relative = true }
            "#,
        )
        .build();

    p.cargo("run").run();
}

#[cargo_test]
fn env_no_override() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("unchanged"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        fn main() {
            println!( "CARGO_PKG_NAME:{}", env!("CARGO_PKG_NAME") );
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                CARGO_PKG_NAME = { value = "from-config", force = true }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
CARGO_PKG_NAME:unchanged

"#]])
        .run();
}

#[cargo_test]
fn env_applied_to_target_info_discovery_rustc() {
    let wrapper = project()
        .at("wrapper")
        .file("Cargo.toml", &basic_manifest("wrapper", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                let mut cmd = std::env::args().skip(1).collect::<Vec<_>>();
                // This will be invoked twice (with `-vV` and with all the `--print`),
                // make sure the environment variable exists each time.
                let env_test = std::env::var("ENV_TEST").unwrap();
                eprintln!("WRAPPER ENV_TEST:{env_test}");
                let (prog, args) = cmd.split_first().unwrap();
                let status = std::process::Command::new(prog)
                    .args(args).status().unwrap();
                std::process::exit(status.code().unwrap_or(1));
            }
            "#,
        )
        .build();
    wrapper.cargo("build").run();
    let wrapper = &wrapper.bin("wrapper");

    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                eprintln!( "MAIN ENV_TEST:{}", std::env!("ENV_TEST") );
            }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST = "from-config"
            "#,
        )
        .build();

    p.cargo("run")
        .env("RUSTC_WORKSPACE_WRAPPER", wrapper)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
WRAPPER ENV_TEST:from-config
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`
MAIN ENV_TEST:from-config

"#]])
        .run();

    // Ensure wrapper also maintains the same overridden priority for envs.
    p.cargo("clean").run();
    p.cargo("run")
        .env("ENV_TEST", "from-env")
        .env("RUSTC_WORKSPACE_WRAPPER", wrapper)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
WRAPPER ENV_TEST:from-env
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`
MAIN ENV_TEST:from-env

"#]])
        .run();
}

#[cargo_test]
fn env_changed_defined_in_config_toml() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        fn main() {
            println!( "{}", env!("ENV_TEST") );
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST = "from-config"
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
from-config

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    p.cargo("run")
        .env("ENV_TEST", "from-env")
        .with_stdout_data(str![[r#"
from-env

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
    // This identical cargo invocation is to ensure no rebuild happen.
    p.cargo("run")
        .env("ENV_TEST", "from-env")
        .with_stdout_data(str![[r#"
from-env

"#]])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn forced_env_changed_defined_in_config_toml() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        fn main() {
            println!( "{}", env!("ENV_TEST") );
        }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [env]
                ENV_TEST = {value = "from-config", force = true}
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
from-config

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    p.cargo("run")
        .env("ENV_TEST", "from-env")
        .with_stdout_data(str![[r#"
from-config

"#]])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn env_changed_defined_in_config_args() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            r#"
        use std::env;
        fn main() {
            println!( "{}", env!("ENV_TEST") );
        }
        "#,
        )
        .build();
    p.cargo(r#"run --config 'env.ENV_TEST="one"'"#)
        .with_stdout_data(str![[r#"
one

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    p.cargo(r#"run --config 'env.ENV_TEST="two"'"#)
        .with_stdout_data(str![[r#"
two

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
    // This identical cargo invocation is to ensure no rebuild happen.
    p.cargo(r#"run --config 'env.ENV_TEST="two"'"#)
        .with_stdout_data(str![[r#"
two

"#]])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}
