//! Tests for natvis Cargo.toml syntax

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name =  "foo"
                version = "0.0.1"

                [debug-visualizations]
                natvis = ["foo.natvis"]
            "#,
        )
        .file("src/main.rs", "fn main() { assert!(true) }")
        .build();

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("[..]feature `natvis` is required")
        .run();
}

#[cargo_test]
fn natvis_file_does_not_exist() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["natvis"]

                [project]
                name =  "foo"
                version = "0.0.1"

                [debug-visualizations]
                natvis = ["foo.natvis"]
            "#,
        )
        .file("src/main.rs", "fn main() { assert!(true) }")
        .build();

    let natvis_path = p.root().join("foo.natvis").display().to_string();
    let expected_msg = format!("[..]incorrect value `{}` for codegen option `natvis`[..]", natvis_path);

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains(expected_msg)
        .run();
}

#[cargo_test]
fn invalid_natvis_extension() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["natvis"]

                [project]
                name =  "foo"
                version = "0.0.1"

                [debug-visualizations]
                natvis = ["foo.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() { assert!(true) }")
        .build();

    let natvis_path = p.root().join("foo.rs").display().to_string();
    let expected_msg = format!("[..]incorrect value `{}` for codegen option `natvis`[..]", natvis_path);

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains(expected_msg)
        .run();
}

#[cargo_test]
fn simple_test() {
    let p = project()
    .file(
        "Cargo.toml",
        r#"
            cargo-features = ["natvis"]

            [project]
            name =  "foo"
            version = "0.0.1"

            [debug-visualizations]
            natvis = ["foo.natvis"]
        "#,
    )
    .file(
        "src/main.rs",
        r#"
            fn main() { assert!(true) }

            pub struct Foo {
                pub x: i32,
                pub y: i32,
                pub z: i32
            }
        "#,
    ).file(
        "foo.natvis",
        r#"
            <?xml version="1.0" encoding="utf-8"?>
            <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
            <Type Name="foo::Foo">
                <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                <Expand>
                    <Item Name="[x]">x</Item>
                    <Item Name="[y]">y</Item>
                    <Item Name="[z]">z</Item>
                </Expand>
            </Type>
            </AutoVisualizer>
        "#,
    ).build();

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn direct_dependency_with_natvis() {
    let p = project()
    .file(
        "Cargo.toml",
        r#"
            [project]
            name =  "foo"
            version = "0.0.1"

            [dependencies]
            test_dependency = { path = "src/test_dependency" }
        "#,
    )
    .file(
        "src/main.rs",
        r#"
            fn main() { assert!(true) }
        "#,
    )
    .file(
        "src/test_dependency/src/lib.rs",
        r#"
            pub struct Foo {
                pub x: i32,
                pub y: i32,
                pub z: i32
            }
        "#)
    .file(
        "src/test_dependency/Cargo.toml",
        r#"
            cargo-features = ["natvis"]

            [project]
            name =  "test_dependency"
            version = "0.0.1"

            [debug-visualizations]
            natvis = ["test_dependency.natvis"]
        "#,
    )
    .file(
        "src/test_dependency/test_dependency.natvis",
        r#"
            <?xml version="1.0" encoding="utf-8"?>
            <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
            <Type Name="test_dependency::Foo">
                <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                <Expand>
                    <Item Name="[x]">x</Item>
                    <Item Name="[y]">y</Item>
                    <Item Name="[z]">z</Item>
                </Expand>
            </Type>
            </AutoVisualizer>
        "#,
    ).build();

    let natvis_path = p.root().join("src/test_dependency/test_dependency.natvis").display().to_string();
    
    // Run cargo build.
    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(0)
        .with_stderr_contains(format!("[..]-C natvis={}[..]", natvis_path))
        .run();
}

#[cargo_test]
fn multiple_natvis_files() {
    let p = project()
    .file(
        "Cargo.toml",
        r#"
            cargo-features = ["natvis"]

            [project]
            name =  "foo"
            version = "0.0.1"

            [debug-visualizations]
            natvis = ["bar.natvis", "foo.natvis"]
        "#,
    )
    .file(
        "src/main.rs",
        r#"
            fn main() { assert!(true) }

            pub struct Foo {
                pub x: i32,
                pub y: i32,
                pub z: i32
            }

            pub struct Bar {
                pub x: i32,
                pub y: i32,
                pub z: i32
            }
        "#,
    )
    .file(
        "foo.natvis",
        r#"
            <?xml version="1.0" encoding="utf-8"?>
            <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
            <Type Name="foo::Foo">
                <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                <Expand>
                    <Item Name="[x]">x</Item>
                    <Item Name="[y]">y</Item>
                    <Item Name="[z]">z</Item>
                </Expand>
            </Type>
            </AutoVisualizer>
        "#,
    )
    .file(
        "bar.natvis",
        r#"
            <?xml version="1.0" encoding="utf-8"?>
            <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
            <Type Name="foo::Bar">
                <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                <Expand>
                    <Item Name="[x]">x</Item>
                    <Item Name="[y]">y</Item>
                    <Item Name="[z]">z</Item>
                </Expand>
            </Type>
            </AutoVisualizer>
        "#,
    ).build();

    let bar_natvis_path = p.root().join("bar.natvis").display().to_string();
    let foo_natvis_path = p.root().join("foo.natvis").display().to_string();
    
    // Run cargo build.
    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(0)
        .with_stderr_contains(format!("[..]-C natvis={},{}[..]", bar_natvis_path, foo_natvis_path))
        .run();
}

#[cargo_test]
fn indirect_dependency_with_natvis() {
    let p = project()
    .file(
        "Cargo.toml",
        r#"
            cargo-features = ["natvis"]

            [project]
            name =  "foo"
            version = "0.0.1"

            [debug-visualizations]
            natvis = ["foo.natvis"]

            [dependencies]
            test_dependency = { path = "src/test_dependency" }
        "#,
    )
    .file(
        "src/main.rs",
        r#"
            fn main() { assert!(true) }

            pub struct Foo {
                pub x: i32,
                pub y: i32,
                pub z: i32
            }
        "#,
    )
    .file(
        "foo.natvis",
        r#"
            <?xml version="1.0" encoding="utf-8"?>
            <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
            <Type Name="foo::Foo">
                <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                <Expand>
                    <Item Name="[x]">x</Item>
                    <Item Name="[y]">y</Item>
                    <Item Name="[z]">z</Item>
                </Expand>
            </Type>
            </AutoVisualizer>
        "#,
    )
    .file(
        "src/test_dependency/Cargo.toml",
        r#"
            [project]
            name =  "test_dependency"
            version = "0.0.1"

            [dependencies]
            nested_dependency = { path = "src/nested_dependency" }
        "#,
    )
    .file("src/test_dependency/src/lib.rs",r#""#,)
    .file(
        "src/test_dependency/src/nested_dependency/Cargo.toml",
        r#"
            cargo-features = ["natvis"]

            [project]
            name =  "nested_dependency"
            version = "0.0.1"

            [debug-visualizations]
            natvis = ["nested_dependency.natvis"]
        "#,
    )
    .file(
        "src/test_dependency/src/nested_dependency/src/lib.rs",
        r#"
            pub struct Bar {
                pub x: i32,
                pub y: i32,
                pub z: i32
            }
        "#,
    )
    .file(
        "src/test_dependency/src/nested_dependency/nested_dependency.natvis",
        r#"
            <?xml version="1.0" encoding="utf-8"?>
            <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
            <Type Name="nested_dependency::Bar">
                <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                <Expand>
                    <Item Name="[x]">x</Item>
                    <Item Name="[y]">y</Item>
                    <Item Name="[z]">z</Item>
                </Expand>
            </Type>
            </AutoVisualizer>
        "#,
    ).build();

    let foo_natvis_path = p.root().join("foo.natvis").display().to_string();
    let nested_dependency_natvis_path = p.root().join("src/test_dependency/src/nested_dependency/nested_dependency.natvis").display().to_string();
    
    // Run cargo build.
    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(0)
        .with_stderr_contains(format!("[..]-C natvis={}[..]", foo_natvis_path))
        .with_stderr_contains(format!("[..]-C natvis={}[..]", nested_dependency_natvis_path))
        .run();
}

#[cargo_test]
fn registry_dependency_natvis() {
    Package::new("bar", "0.0.1")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["natvis"]

                [project]
                name = "bar"
                version = "0.0.1"

                [debug-visualizations]
                natvis = ["bar.natvis"]
            "#,
        )
        .file(
            "bar.natvis",
            r#"
                <?xml version="1.0" encoding="utf-8"?>
                <AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
                <Type Name="bar::Bar">
                    <DisplayString>x:{x}, y:{y}, z:{z}</DisplayString>
                    <Expand>
                        <Item Name="[x]">x</Item>
                        <Item Name="[y]">y</Item>
                        <Item Name="[z]">z</Item>
                    </Expand>
                </Type>
                </AutoVisualizer>
            "#,
        )
        .file("src/lib.rs","")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(format!("[..]-C natvis=[..]/bar.natvis[..]"))
        .run();
}