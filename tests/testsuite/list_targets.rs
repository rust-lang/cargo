//! Tests for target filter flags giving suggestions on which targets are available.

use cargo_test_support::project;

const EXAMPLE: u8 = 0x1;
const BIN: u8 = 0x2;
const TEST: u8 = 0x4;
const BENCH: u8 = 0x8;

fn list_targets_test(command: &str, targets: u8) {
    let full_project = project()
        .file("examples/a.rs", "fn main() { }")
        .file("examples/b.rs", "fn main() { }")
        .file("benches/bench1.rs", "")
        .file("benches/bench2.rs", "")
        .file("tests/test1.rs", "")
        .file("tests/test2.rs", "")
        .file("src/main.rs", "fn main() { }")
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
}

#[cargo_test]
fn build_list_targets() {
    list_targets_test("build", EXAMPLE | BIN | TEST | BENCH);
}

#[cargo_test]
fn check_list_targets() {
    list_targets_test("check", EXAMPLE | BIN | TEST | BENCH);
}

#[cargo_test]
fn doc_list_targets() {
    list_targets_test("doc", BIN);
}

#[cargo_test]
fn fix_list_targets() {
    list_targets_test("fix", EXAMPLE | BIN | TEST | BENCH);
}

#[cargo_test]
fn run_list_targets() {
    list_targets_test("run", EXAMPLE | BIN);
}

#[cargo_test]
fn test_list_targets() {
    list_targets_test("test", EXAMPLE | BIN | TEST | BENCH);
}

#[cargo_test]
fn bench_list_targets() {
    list_targets_test("bench", EXAMPLE | BIN | TEST | BENCH);
}

#[cargo_test]
fn install_list_targets() {
    list_targets_test("install", EXAMPLE | BIN);
}

#[cargo_test]
fn rustdoc_list_targets() {
    list_targets_test("rustdoc", EXAMPLE | BIN | TEST | BENCH);
}

#[cargo_test]
fn rustc_list_targets() {
    list_targets_test("rustc", EXAMPLE | BIN | TEST | BENCH);
}
