use crate::support::project;

#[cargo_test]
fn syntax() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg(unix)'.features]
            b = []
            [target.'cfg(windows)'.features]
            b = []
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn bb() {}
        "#,
        )
        .build();
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] a v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn include_by_param() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg(unix)'.features]
            b = []
            [target.'cfg(windows)'.features]
            c = []
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "b")]
            pub const BB: usize = 0;
            #[cfg(feature = "c")]
            pub const BB: usize = 1;
            
            pub fn bb() -> Result<(), ()> { if BB > 0 { Ok(()) } else { Err(()) } }
        "#,
        )
        .build();
    p.cargo(format!("build --features {}", if cfg!(unix) { "b" } else { "c" }).as_str())
        .with_stderr(
            "\
[COMPILING] a v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn dont_include_by_platform() {
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

            [target.'cfg({})'.features]
            b = []
        "#,
                other_family
            ),
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "b")]
            pub const BB: usize = 0;
            
            pub fn bb() { let _ = BB; }
        "#,
        )
        .build();
    p.cargo("build --features b -vv")
        .with_status(101)
        .with_stderr_contains(
            "\
             error[E0425]: cannot find value `BB` in this scope",
        )
        .run();
}

#[cargo_test]
fn dont_include_by_param() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.'cfg(unix)'.features]
            b = []
            [target.'cfg(windows)'.features]
            c = []
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "b")]
            pub const BB: usize = 0;
            #[cfg(feature = "c")]
            pub const BB: usize = 1;
            
            pub fn bb() -> Result<(), ()> { if BB > 0 { Ok(()) } else { Err(()) } }
        "#,
        )
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(
            "\
             error[E0425]: cannot find value `BB` in this scope",
        )
        .run();
}

#[cargo_test]
fn dont_include_default() {
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
    
                [target.'cfg({})'.features]
                b = []
                
                [features]
                default = ["b"]
            "#,
                other_family
            ),
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "b")]
            pub const BB: usize = 0;
            
            pub fn bb() { let _ = BB; }
        "#,
        )
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(
            "\
             error[E0425]: cannot find value `BB` in this scope",
        )
        .run();
}

#[cargo_test]
fn transitive() {
    #[cfg(target_os = "macos")]
    let config = "target_os = \"macos\"";
    #[cfg(target_os = "windows")]
    let config = "target_os = \"windows\"";
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let config = "unix";

    let p = project()
        .no_manifest()
        // root depends on a and c="1.1.0"
        .file(
            "root/Cargo.toml",
            r#"
            [package]
			name = "root"
			version = "0.0.1"
			authors = []
			
			[dependencies]
			a = { version = "*", path = "../a" }
			c = { version = "1.1.0", path = "../c1" }
        "#,
        )
        .file(
            "root/src/main.rs",
            r#"
            fn main() {
			    println!("Hello, world!");
			}
        "#,
        )
        // a depends on b and on OSX depends on b's flag maybe
        .file(
            "a/Cargo.toml",
            &format!(
                r#"
				[package]
				name = "a"
				version = "0.1.0"
				
				[lib]
				name = "a"
				
				[target.'cfg(not({}))'.dependencies]
				b = {{ version = "*", path = "../b" }}
				
				[target.'cfg({})'.dependencies]
				b = {{ version = "*", path = "../b", features = ["maybe"] }}
		        "#,
                config, config,
            ),
        )
        .file(
            "a/src/lib.rs",
            r#"
        "#,
        )
        // b depends on c="=1.0.0" if maybe is active.
        .file(
            "b/Cargo.toml",
            r#"
			[package]
			name = "b"
			version = "0.1.0"
			
			[dependencies]
			c = { version = "1.0.0", path = "../c0", optional = true }
			
			[features]
			maybe = ["c"]
		"#,
        )
        .file(
            "b/src/lib.rs",
            r#"
			#[cfg(feature = "maybe")]
			pub fn maybe() {
				c::cee();
			}
        "#,
        )
        // c 1.0.0
        .file(
            "c0/Cargo.toml",
            r#"
			[package]
			name = "c"
			version = "1.0.0"
			
			[lib]
			name = "c"
			
			[dependencies]
        "#,
        )
        .file(
            "c0/src/lib.rs",
            r#"			
			pub fn cee() {}
        "#,
        )
        // c 1.1.0
        .file(
            "c1/Cargo.toml",
            r#"
			[package]
			name = "c"
			version = "1.1.0"
			
			[lib]
			name = "c"
			
			[dependencies]
        "#,
        )
        .file(
            "c1/src/lib.rs",
            r#"
        "#,
        )
        .build();

    p.cargo("build")
        .cwd("root")
        .with_stderr(
            "\
[COMPILING] c v1.0.0 ([..])
[COMPILING] c v1.1.0 ([..])
[COMPILING] b v0.1.0 ([..])
[COMPILING] a v0.1.0 ([..])
[COMPILING] root v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
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

            [target.'cfg(with_b)'.features]
            b = []
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[cfg(with_b)]
            pub const BB: usize = 0;

            fn main() { let _ = BB; }
        "#,
        )
        .build();

    p.cargo("build --target x86_64-unknown-linux-gnu")
        .env("RUSTFLAGS", "--cfg with_b")
        .run();
}
