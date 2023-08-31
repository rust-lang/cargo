//! Tests for packages/target filter flags giving suggestions on which
//! packages/targets are available.

use cargo_test_support::project;

const EXAMPLE: u8 = 1 << 0;
const BIN: u8 = 1 << 1;
const TEST: u8 = 1 << 2;
const BENCH: u8 = 1 << 3;
const PACKAGE: u8 = 1 << 4;
const TARGET: u8 = 1 << 5;

fn list_availables_test(command: &str, targets: u8) {
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

    if targets & EXAMPLE != 0 {
        full_project
            .cargo(&format!("{} --example", command))
            .with_stderr(
                "\
error: \"--example\" takes one argument.
Available examples:
    a
    b

",
            )
            .with_status(101)
            .run();
    }

    if targets & BIN != 0 {
        full_project
            .cargo(&format!("{} --bin", command))
            .with_stderr(
                "\
error: \"--bin\" takes one argument.
Available binaries:
    foo

",
            )
            .with_status(101)
            .run();
    }

    if targets & BENCH != 0 {
        full_project
            .cargo(&format!("{} --bench", command))
            .with_stderr(
                "\
error: \"--bench\" takes one argument.
Available benches:
    bench1
    bench2

",
            )
            .with_status(101)
            .run();
    }

    if targets & TEST != 0 {
        full_project
            .cargo(&format!("{} --test", command))
            .with_stderr(
                "\
error: \"--test\" takes one argument.
Available tests:
    test1
    test2

",
            )
            .with_status(101)
            .run();
    }

    if targets & PACKAGE != 0 {
        full_project
            .cargo(&format!("{} -p", command))
            .with_stderr(
                "\
[ERROR] \"--package <SPEC>\" requires a SPEC format value, \
which can be any package ID specifier in the dependency graph.
Run `cargo help pkgid` for more information about SPEC format.

Possible packages/workspace members:
    foo

",
            )
            .with_status(101)
            .run();
    }

    let empty_project = project().file("src/lib.rs", "").build();

    if targets & EXAMPLE != 0 {
        empty_project
            .cargo(&format!("{} --example", command))
            .with_stderr(
                "\
error: \"--example\" takes one argument.
No examples available.

",
            )
            .with_status(101)
            .run();
    }

    if targets & BIN != 0 {
        empty_project
            .cargo(&format!("{} --bin", command))
            .with_stderr(
                "\
error: \"--bin\" takes one argument.
No binaries available.

",
            )
            .with_status(101)
            .run();
    }

    if targets & BENCH != 0 {
        empty_project
            .cargo(&format!("{} --bench", command))
            .with_stderr(
                "\
error: \"--bench\" takes one argument.
No benches available.

",
            )
            .with_status(101)
            .run();
    }

    if targets & TEST != 0 {
        empty_project
            .cargo(&format!("{} --test", command))
            .with_stderr(
                "\
error: \"--test\" takes one argument.
No tests available.

",
            )
            .with_status(101)
            .run();
    }

    if targets & TARGET != 0 {
        empty_project
            .cargo(&format!("{} --target", command))
            .with_stderr(
                "\
error: \"--target\" takes a target architecture as an argument.

Run `[..]` to see possible targets.
",
            )
            .with_status(101)
            .run();
    }
}

#[cargo_test]
fn build_list_availables() {
    list_availables_test("build", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn check_list_availables() {
    list_availables_test("check", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn doc_list_availables() {
    list_availables_test("doc", BIN | PACKAGE | TARGET);
}

#[cargo_test]
fn fix_list_availables() {
    list_availables_test("fix", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn run_list_availables() {
    list_availables_test("run", EXAMPLE | BIN | PACKAGE | TARGET);
}

#[cargo_test]
fn test_list_availables() {
    list_availables_test("test", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn bench_list_availables() {
    list_availables_test("bench", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn install_list_availables() {
    list_availables_test("install", EXAMPLE | BIN | TARGET);
}

#[cargo_test]
fn rustdoc_list_availables() {
    list_availables_test("rustdoc", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn rustc_list_availables() {
    list_availables_test("rustc", EXAMPLE | BIN | TEST | BENCH | PACKAGE | TARGET);
}

#[cargo_test]
fn pkgid_list_availables() {
    list_availables_test("pkgid", PACKAGE);
}

#[cargo_test]
fn tree_list_availables() {
    list_availables_test("tree", PACKAGE | TARGET);
}

#[cargo_test]
fn clean_list_availables() {
    list_availables_test("clean", PACKAGE | TARGET);
}

#[cargo_test]
fn update_list_availables() {
    list_availables_test("update", PACKAGE);
}
