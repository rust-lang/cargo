//! Tests for `[env]` config.

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
            ".cargo/config",
            r#"
                [env]
                ENV_TEST_1233 = "Hello"
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_contains("compile-time:Hello")
        .with_stdout_contains("run-time:Hello")
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
            ".cargo/config",
            r#"
                [env]
                ENV_TEST_BOOL = false
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]could not load config key `env.ENV_TEST_BOOL`")
        .run();
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
            ".cargo/config",
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
        .with_stdout_contains("ENV_TEST_FORCED:from-config")
        .with_stdout_contains("ENV_TEST_UNFORCED:from-env")
        .with_stdout_contains("ENV_TEST_UNFORCED_DEFAULT:from-env")
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
            ".cargo/config",
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
            ".cargo/config",
            r#"
                [env]
                CARGO_PKG_NAME = { value = "from-config", force = true }
            "#,
        )
        .build();

    p.cargo("run -Zconfigurable-env")
        .masquerade_as_nightly_cargo()
        .with_stdout_contains("CARGO_PKG_NAME:unchanged")
        .run();
}
