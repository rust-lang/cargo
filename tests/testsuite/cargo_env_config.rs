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
fn env_external_subcommand() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("cargo-fake-subcommand"))
        .file(
            "src/main.rs",
            r#"
            use std::env;
            fn main() {
                // ENV_TEST_SUB should be available to the build AND to the subcommand.
                assert_eq!(env!("ENV_TEST_SUB"), "TEST_VALUE");
                assert_eq!(&env::var("ENV_TEST_SUB").unwrap(), "TEST_VALUE");

                // ENV_TEST_NOSUB should be available to the build, but not to the subcommand.
                assert!(env::var_os("ENV_TEST_NOSUB").is_none());
                assert_eq!(env!("ENV_TEST_NOSUB"), "TEST_VALUE");
            }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
                [env]
                ENV_TEST_SUB = "TEST_VALUE"
                ENV_TEST_NOSUB = { value = "TEST_VALUE", apply_to_subcommands = false }
            "#,
        )
        .build();
    p.cargo("install --path .").run();
    p.cargo("fake-subcommand").run();
}

#[cargo_test]
fn env_no_cargo_vars() {
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
                CARGO_HOME = { value = "/dev/null", force = true }
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr("[..]setting CARGO_ variables from [env] is not allowed.")
        .run();
}

#[cargo_test]
fn env_build_script() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/main.rs",
            "fn main() {}",
        )
        .file(
            "build.rs",
            r#"
                use std::env;

                fn main() {
                    // env should be set during the build script's build and execution.
                    assert_eq!(env!("ENV_TEST_VAR"), "TEST_VAR_VALUE");
                    assert_eq!(env::var("ENV_TEST_VAR").unwrap(), "TEST_VAR_VALUE");
                }
            "#
        )
        .file(
            ".cargo/config",
            r#"
                [env]
                ENV_TEST_VAR = "TEST_VAR_VALUE"
            "#,
        )
        .build();

    p.cargo("build")
        .run();
}
