//! Tests for `[env]` config.

use cargo_test_support::{basic_bin_manifest, project};

#[cargo_test]
fn env_basic() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
        use std::env;
        fn main() {
            println!( "compile-time:{}", env!("ENV_TEST_1233") );
            println!( "run-time:{}", env::var("ENV_TEST_1233").unwrap());
        }
        "#)
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
        .file("src/main.rs", r#"
        fn main() {
        }
        "#)
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
        .with_stderr_contains("[..]`env.ENV_TEST_BOOL` expected a string, but found a boolean")
        .run();
}

#[cargo_test]
fn env_force() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
        use std::env;
        fn main() {
            println!( "ENV_TEST_FORCED:{}", env!("ENV_TEST_FORCED") );
            println!( "ENV_TEST_UNFORCED:{}", env!("ENV_TEST_UNFORCED") );
        }
        "#)
        .file(
        ".cargo/config",
        r#"
                [env]
                ENV_TEST_UNFORCED = "from-config"
                ENV_TEST_FORCED = { value = "from-config", force = true }
            "#,
        )
        .build();

    p.cargo("run")
        .env("ENV_TEST_FORCED", "from-env")
        .env("ENV_TEST_UNFORCED", "from-env")
        .with_stdout_contains("ENV_TEST_FORCED:from-config")
        .with_stdout_contains("ENV_TEST_UNFORCED:from-env")
        .run();
}

#[cargo_test]
fn env_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo2"))
        .file("src/main.rs", r#"
        use std::env;
        use std::path::Path;
        fn main() {
            println!( "ENV_TEST_RELATIVE:{}", env!("ENV_TEST_RELATIVE") );
            println!( "ENV_TEST_ABSOLUTE:{}", env!("ENV_TEST_ABSOLUTE") );

            assert!( Path::new(env!("ENV_TEST_ABSOLUTE")).is_absolute() );
            assert!( !Path::new(env!("ENV_TEST_RELATIVE")).is_absolute() );
        }
        "#)
        .file(
        ".cargo/config",
        r#"
                [env]
                ENV_TEST_RELATIVE = "Cargo.toml"
                ENV_TEST_ABSOLUTE = { value = "Cargo.toml", relative = true }
            "#,
        )
        .build();

    p.cargo("run")
        .run();
}
