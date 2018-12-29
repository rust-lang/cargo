use crate::support::project;

// cargo {run,install} only support --example and --bin
// cargo {build,check,fix,test} support --example, --bin, --bench and --test

fn test_list_targets_example_and_bin_only(command: &str) {
    let p = project()
        .file("examples/a.rs", "fn main() { }")
        .file("examples/b.rs", "fn main() { }")
        .file("src/main.rs", "fn main() { }")
        .build();

    p.cargo(&format!("{} --example", command))
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

    p.cargo(&format!("{} --bin", command))
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

fn test_empty_list_targets_example_and_bin_only(command: &str) {
    let p = project().file("src/lib.rs", "").build();

    p.cargo(&format!("{} --example", command))
        .with_stderr(
            "\
error: \"--example\" takes one argument.
No examples available.

",
        )
        .with_status(101)
        .run();

    p.cargo(&format!("{} --bin", command))
        .with_stderr(
            "\
error: \"--bin\" takes one argument.
No binaries available.

",
        )
        .with_status(101)
        .run();
}

fn test_list_targets_full(command: &str) {
    let p = project()
        .file("examples/a.rs", "fn main() { }")
        .file("examples/b.rs", "fn main() { }")
        .file("benches/bench1.rs", "")
        .file("benches/bench2.rs", "")
        .file("tests/test1.rs", "")
        .file("tests/test2.rs", "")
        .file("src/main.rs", "fn main() { }")
        .build();

    p.cargo(&format!("{} --example", command))
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

    p.cargo(&format!("{} --bin", command))
        .with_stderr(
            "\
error: \"--bin\" takes one argument.
Available binaries:
    foo

",
        )
        .with_status(101)
        .run();

    p.cargo(&format!("{} --bench", command))
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

    p.cargo(&format!("{} --test", command))
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

fn test_empty_list_targets_full(command: &str) {
    let p = project().file("src/lib.rs", "").build();

    p.cargo(&format!("{} --example", command))
        .with_stderr(
            "\
error: \"--example\" takes one argument.
No examples available.

",
        )
        .with_status(101)
        .run();

    p.cargo(&format!("{} --bin", command))
        .with_stderr(
            "\
error: \"--bin\" takes one argument.
No binaries available.

",
        )
        .with_status(101)
        .run();

    p.cargo(&format!("{} --bench", command))
        .with_stderr(
            "\
error: \"--bench\" takes one argument.
No benches available.

",
        )
        .with_status(101)
        .run();

    p.cargo(&format!("{} --test", command))
        .with_stderr(
            "\
error: \"--test\" takes one argument.
No tests available.

",
        )
        .with_status(101)
        .run();
}

#[test]
fn build_list_targets() {
    test_list_targets_full("build");
}
#[test]
fn build_list_targets_empty() {
    test_empty_list_targets_full("build");
}

#[test]
fn check_list_targets() {
    test_list_targets_full("check");
}
#[test]
fn check_list_targets_empty() {
    test_empty_list_targets_full("check");
}

#[test]
fn fix_list_targets() {
    test_list_targets_full("fix");
}
#[test]
fn fix_list_targets_empty() {
    test_empty_list_targets_full("fix");
}

#[test]
fn run_list_targets() {
    test_list_targets_example_and_bin_only("run");
}
#[test]
fn run_list_targets_empty() {
    test_empty_list_targets_example_and_bin_only("run");
}

#[test]
fn test_list_targets() {
    test_list_targets_full("test");
}
#[test]
fn test_list_targets_empty() {
    test_empty_list_targets_full("test");
}

#[test]
fn bench_list_targets() {
    test_list_targets_full("bench");
}
#[test]
fn bench_list_targets_empty() {
    test_empty_list_targets_full("bench");
}

#[test]
fn install_list_targets() {
    test_list_targets_example_and_bin_only("install");
}
#[test]
fn install_list_targets_empty() {
    test_empty_list_targets_example_and_bin_only("install");
}
