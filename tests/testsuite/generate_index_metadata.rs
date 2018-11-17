use crate::support::project;
use crate::support::registry::{self, Package};

#[test]
fn missing_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-index-metadata")
        .with_status(101)
        .with_stderr(&format!(
            "\
error: could not find package file. Ensure that crate has been packaged using `cargo package`

Caused by:
  failed to open: {path}

Caused by:
  [..]
",
            path = p
                .root()
                .join("target")
                .join("package")
                .join("foo-0.0.1.crate")
                .display()
        ))
        .run();
}

#[test]
fn simple_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("package").run();

    p.cargo("generate-index-metadata")
        .with_json(
            r#"
        {
            "name": "foo",
            "vers": "0.0.1",
            "deps": [],
            "features": {},
            "cksum": "[..]",
            "yanked": false
        }"#,
        )
        .run();
}

#[test]
fn project_with_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'

            [dependencies]
            bar = "0.0.4"
            baz = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.2").publish();
    Package::new("nested", "0.0.3").publish();
    Package::new("bar", "0.0.4").dep("nested", "*").publish();

    p.cargo("package").run();

    p.cargo("generate-index-metadata")
        .with_json(
            r#"
        {
            "name": "foo",
            "vers": "0.0.1",
            "deps": [
                {
                    "default_features": true,
                    "features": [],
                    "kind": "normal",
                    "name": "bar",
                    "optional": false,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "^0.0.4",
                    "target": null
                },
                {
                    "default_features": true,
                    "features": [],
                    "kind": "normal",
                    "name": "baz",
                    "optional": false,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                }
            ],
            "features": {},
            "cksum": "[..]",
            "yanked": false
        }"#,
        )
        .run();
}

#[test]
fn project_with_dev_and_build_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'

            [dev-dependencies]
            bar = "*"

            [build-dependencies]
            baz= { version = "*", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").publish();

    p.cargo("package").run();

    p.cargo("generate-index-metadata")
        .with_json(
            r#"
        {
            "name": "foo",
            "vers": "0.0.1",
            "deps": [
                {
                    "default_features": true,
                    "features": [],
                    "kind": "dev",
                    "name": "bar",
                    "optional": false,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                },
                {
                    "default_features": true,
                    "features": [],
                    "kind": "build",
                    "name": "baz",
                    "optional": true,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                }
            ],
            "features": {},
            "cksum": "[..]",
            "yanked": false
        }"#,
        )
        .run();
}

#[test]
fn project_with_deps_from_alternative_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'

            [dependencies]
            bar = "*"
            baz = { version = "*", registry = "alternative" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").alternative(true).publish();
    Package::new("bar", "0.0.1").publish();

    p.cargo("package").masquerade_as_nightly_cargo().run();

    p.cargo("generate-index-metadata")
        .masquerade_as_nightly_cargo()
        .with_json(&format!(
            r#"
            {{
                "name": "foo",
                "vers": "0.0.1",
                "deps": [
                    {{
                        "default_features": true,
                        "features": [],
                        "kind": "normal",
                        "name": "bar",
                        "optional": false,
                        "registry": "https://github.com/rust-lang/crates.io-index",
                        "req": "*",
                        "target": null
                    }},
                    {{
                        "default_features": true,
                        "features": [],
                        "kind": "normal",
                        "name": "baz",
                        "optional": false,
                        "registry": "{reg}",
                        "req": "*",
                        "target": null
                    }}
                ],
                "features": {{}},
                "cksum": "[..]",
                "yanked": false
            }}"#,
            reg = registry::alt_registry()
        ))
        .run();
}

#[test]
fn project_with_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'

            [features]
            default = ["jquery", "session"]
            go-faster = []
            secure-password = ["bcrypt"]
            session = ["cookie/session"]

            [dependencies]
            cookie = "*"
            jquery = { version = "*", optional = true }
            bcrypt = { version = "*", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("cookie", "0.0.1")
        .feature("session", &[])
        .publish();
    Package::new("jquery", "0.0.1").publish();
    Package::new("bcrypt", "0.0.1").publish();

    p.cargo("package").run();

    p.cargo("generate-index-metadata")
        .with_json(
            r#"
        {
            "name": "foo",
            "vers": "0.0.1",
            "deps": [
                {
                    "default_features": true,
                    "features": [],
                    "kind": "normal",
                    "name": "bcrypt",
                    "optional": true,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                },
                {
                    "default_features": true,
                    "features": [],
                    "kind": "normal",
                    "name": "cookie",
                    "optional": false,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                },
                {
                    "default_features": true,
                    "features": [],
                    "kind": "normal",
                    "name": "jquery",
                    "optional": true,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                }
            ],
            "features": {
                "default": [
                    "jquery",
                    "session"
                ],
                "go-faster": [],
                "secure-password": [
                    "bcrypt"
                ],
                "session": [
                    "cookie/session"
                ]
            },
            "cksum": "[..]",
            "yanked": false
        }"#,
        )
        .run();
}

#[test]
fn project_with_dep_with_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'

            [dependencies]
            bar = { version = "*", default-features = false, features = ["baz"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").feature("baz", &[]).publish();

    p.cargo("package").run();

    p.cargo("generate-index-metadata")
        .with_json(
            r#"
        {
            "name": "foo",
            "vers": "0.0.1",
            "deps": [
                {
                    "default_features": false,
                    "features": ["baz"],
                    "kind": "normal",
                    "name": "bar",
                    "optional": false,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": null
                }
            ],
            "features": {},
            "cksum": "[..]",
            "yanked": false
        }"#,
        )
        .run();
}

#[test]
fn project_with_platform_specific_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'

            [target.'cfg(windows)'.dependencies]
            winapi = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("winapi", "0.0.1").publish();

    p.cargo("package").run();

    p.cargo("generate-index-metadata")
        .with_json(
            r#"
        {
            "name": "foo",
            "vers": "0.0.1",
            "deps": [
                {
                    "default_features": true,
                    "features": [],
                    "kind": "normal",
                    "name": "winapi",
                    "optional": false,
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "req": "*",
                    "target": "cfg(windows)"
                }
            ],
            "features": {},
            "cksum": "[..]",
            "yanked": false
        }"#,
        )
        .run();
}
