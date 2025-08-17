//! Tests for packages/target filter flags giving suggestions on which
//! packages/targets are available.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;
use snapbox::IntoData;

fn list_availables_test(command: &str, expected: ExpectedSnapshots<impl IntoData>) {
    let full_project = project()
        .file("examples/a.rs", "fn main() { }")
        .file("examples/b.rs", "fn main() { }")
        .file("benches/bench1.rs", "")
        .file("benches/bench2.rs", "")
        .file("tests/test1.rs", "")
        .file("tests/test2.rs", "")
        .file("src/main.rs", "fn main() { }")
        .file("Cargo.lock", "") // for `cargo pkgid`
        .build();

    if let ExpectedSnapshots {
        example: ProjectExpected {
            full: Some(example),
            ..
        },
        ..
    } = expected
    {
        full_project
            .cargo(&format!("{} --example", command))
            .with_stderr_data(example)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        bin: ProjectExpected {
            full: Some(bin), ..
        },
        ..
    } = expected
    {
        full_project
            .cargo(&format!("{} --bin", command))
            .with_stderr_data(bin)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        bench: ProjectExpected {
            full: Some(bench), ..
        },
        ..
    } = expected
    {
        full_project
            .cargo(&format!("{} --bench", command))
            .with_stderr_data(bench)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        test: ProjectExpected {
            full: Some(test), ..
        },
        ..
    } = expected
    {
        full_project
            .cargo(&format!("{} --test", command))
            .with_stderr_data(test)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        package: ProjectExpected {
            full: Some(package),
            ..
        },
        ..
    } = expected
    {
        full_project
            .cargo(&format!("{} -p", command))
            .with_stderr_data(package)
            .with_status(101)
            .run();
    }

    let empty_project = project().file("src/lib.rs", "").build();

    if let ExpectedSnapshots {
        example: ProjectExpected {
            empty: Some(example),
            ..
        },
        ..
    } = expected
    {
        empty_project
            .cargo(&format!("{} --example", command))
            .with_stderr_data(example)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        bin: ProjectExpected {
            empty: Some(bin), ..
        },
        ..
    } = expected
    {
        empty_project
            .cargo(&format!("{} --bin", command))
            .with_stderr_data(bin)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        bench: ProjectExpected {
            empty: Some(bench), ..
        },
        ..
    } = expected
    {
        empty_project
            .cargo(&format!("{} --bench", command))
            .with_stderr_data(bench)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        test: ProjectExpected {
            empty: Some(test), ..
        },
        ..
    } = expected
    {
        empty_project
            .cargo(&format!("{} --test", command))
            .with_stderr_data(test)
            .with_status(101)
            .run();
    }

    if let ExpectedSnapshots {
        target: ProjectExpected {
            empty: Some(target),
            ..
        },
        ..
    } = expected
    {
        empty_project
            .cargo(&format!("{} --target", command))
            .with_stderr_data(target)
            .with_status(101)
            .run();
    }
}

#[cargo_test]
fn build_list_availables() {
    list_availables_test(
        "build",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn check_list_availables() {
    list_availables_test(
        "check",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn doc_list_availables() {
    list_availables_test(
        "doc",
        SnapshotsBuilder::new()
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn fix_list_availables() {
    list_availables_test(
        "fix",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn run_list_availables() {
    list_availables_test(
        "run",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn test_list_availables() {
    list_availables_test(
        "test",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn bench_list_availables() {
    list_availables_test(
        "bench",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn install_list_availables() {
    list_availables_test(
        "install",
        SnapshotsBuilder::new()
            .with_example(
                str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]],
                str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]],
            )
            .with_bin(
                str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]],
                str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]],
            )
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn rustdoc_list_availables() {
    list_availables_test(
        "rustdoc",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn rustc_list_availables() {
    list_availables_test(
        "rustc",
        SnapshotsBuilder::new()
            .with_example(str![[r#"
[ERROR] "--example" takes one argument.
Available examples:
    a
    b


"#]], str![[r#"
[ERROR] "--example" takes one argument.
No examples available.


"#]])
            .with_bin(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]], str![[r#"
[ERROR] "--bin" takes one argument.
No binaries available.


"#]])
            .with_test(str![[r#"
[ERROR] "--test" takes one argument.
Available test targets:
    test1
    test2


"#]], str![[r#"
[ERROR] "--test" takes one argument.
No test targets available.


"#]])
            .with_bench(str![[r#"
[ERROR] "--bench" takes one argument.
Available bench targets:
    bench1
    bench2


"#]], str![[r#"
[ERROR] "--bench" takes one argument.
No bench targets available.


"#]])
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn pkgid_list_availables() {
    list_availables_test(
        "pkgid",
        SnapshotsBuilder::new()
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .build(),
    );
}

#[cargo_test]
fn tree_list_availables() {
    list_availables_test(
        "tree",
        SnapshotsBuilder::new()
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    )
}

#[cargo_test]
fn clean_list_availables() {
    list_availables_test(
        "clean",
        SnapshotsBuilder::new()
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .with_target(str![[r#"
[ERROR] "--target" takes a target architecture as an argument.

Run `[..]` to see possible targets.

"#]])
            .build(),
    );
}

#[cargo_test]
fn update_list_availables() {
    list_availables_test(
        "update",
        SnapshotsBuilder::new()
            .with_package(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value, which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo


"#]])
            .build(),
    );
}

struct ExpectedSnapshots<T: IntoData> {
    example: ProjectExpected<T>,
    bin: ProjectExpected<T>,
    test: ProjectExpected<T>,
    bench: ProjectExpected<T>,
    package: ProjectExpected<T>,
    target: ProjectExpected<T>,
}

struct ProjectExpected<T: IntoData> {
    full: Option<T>,
    empty: Option<T>,
}

struct SnapshotsBuilder<T: IntoData> {
    example: ProjectExpected<T>,
    bin: ProjectExpected<T>,
    test: ProjectExpected<T>,
    bench: ProjectExpected<T>,
    package: ProjectExpected<T>,
    target: ProjectExpected<T>,
}

impl<T: IntoData> SnapshotsBuilder<T> {
    pub fn new() -> Self {
        Self {
            example: ProjectExpected {
                full: None,
                empty: None,
            },
            bin: ProjectExpected {
                full: None,
                empty: None,
            },
            test: ProjectExpected {
                full: None,
                empty: None,
            },
            bench: ProjectExpected {
                full: None,
                empty: None,
            },
            package: ProjectExpected {
                full: None,
                empty: None,
            },
            target: ProjectExpected {
                full: None,
                empty: None,
            },
        }
    }

    fn with_example(mut self, full: T, empty: T) -> Self {
        self.example.full = Some(full);
        self.example.empty = Some(empty);
        self
    }

    fn with_bin(mut self, full: T, empty: T) -> Self {
        self.bin.full = Some(full);
        self.bin.empty = Some(empty);
        self
    }

    fn with_test(mut self, full: T, empty: T) -> Self {
        self.test.full = Some(full);
        self.test.empty = Some(empty);
        self
    }

    fn with_bench(mut self, full: T, empty: T) -> Self {
        self.bench.full = Some(full);
        self.bench.empty = Some(empty);
        self
    }

    fn with_package(mut self, full: T) -> Self {
        self.package.full = Some(full);
        self
    }

    fn with_target(mut self, empty: T) -> Self {
        self.target.empty = Some(empty);
        self
    }

    fn build(self) -> ExpectedSnapshots<T> {
        ExpectedSnapshots {
            example: self.example,
            bin: self.bin,
            test: self.test,
            bench: self.bench,
            package: self.package,
            target: self.target,
        }
    }
}
