//! Tests for target filter flags with glob patterns.

use crate::prelude::*;
use cargo_test_support::{Project, project, str};

#[cargo_test]
fn build_example() {
    full_project()
        .cargo("build -v --example 'ex*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name example1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_bin() {
    full_project()
        .cargo("build -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_bench() {
    full_project()
        .cargo("build -v --bench 'be*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name bench1 [..]`
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn build_test() {
    full_project()
        .cargo("build -v --test 'te*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name test1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn check_example() {
    full_project()
        .cargo("check -v --example 'ex*1'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name example1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_bin() {
    full_project()
        .cargo("check -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_bench() {
    full_project()
        .cargo("check -v --bench 'be*1'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bench1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_test() {
    full_project()
        .cargo("check -v --test 'te*1'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name test1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn doc_bin() {
    full_project()
        .cargo("doc -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc --edition=2015 --crate-type bin --crate-name bin1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/bin1/index.html

"#]])
        .run();
}

#[cargo_test]
fn fix_example() {
    full_project()
        .cargo("fix -v --example 'ex*1' --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name example1 [..]`
[FIXING] examples/example1.rs
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fix_bin() {
    full_project()
        .cargo("fix -v --bin 'bi*1' --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name bin1 [..]`
[FIXING] src/bin/bin1.rs
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fix_bench() {
    full_project()
        .cargo("fix -v --bench 'be*1' --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name bench1 [..]`
[FIXING] benches/bench1.rs
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fix_test() {
    full_project()
        .cargo("fix -v --test 'te*1' --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name test1 [..]`
[FIXING] tests/test1.rs
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn run_example_and_bin() {
    let p = full_project();
    p.cargo("run -v --bin 'bi*1'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` does not support glob patterns on target selection

"#]])
        .run();

    p.cargo("run -v --example 'ex*1'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` does not support glob patterns on target selection

"#]])
        .run();
}

#[cargo_test]
fn test_example() {
    full_project()
        .cargo("test -v --example 'ex*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name example1 [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/examples/example1-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
fn test_bin() {
    full_project()
        .cargo("test -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/bin1-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
fn test_bench() {
    full_project()
        .cargo("test -v --bench 'be*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name bench1 [..]`
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/bench1-[HASH][EXE]`

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn test_test() {
    full_project()
        .cargo("test -v --test 'te*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name test1 [..]`
[RUNNING] `rustc --crate-name bin1 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name bin2 [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/deps/test1-[HASH][EXE]`

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn bench_example() {
    full_project()
        .cargo("bench -v --example 'ex*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name example1 [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/release/examples/example1-[HASH][EXE] --bench`

"#]])
        .run();
}

#[cargo_test]
fn bench_bin() {
    full_project()
        .cargo("bench -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/release/deps/bin1-[HASH][EXE] --bench`

"#]])
        .run();
}

#[cargo_test]
fn bench_bench() {
    full_project()
        .cargo("bench -v --bench 'be*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name bench1 [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/release/deps/bench1-[HASH][EXE] --bench`

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn bench_test() {
    full_project()
        .cargo("bench -v --test 'te*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name test1 [..]`
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/release/deps/test1-[HASH][EXE] --bench`

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn install_example() {
    full_project()
        .cargo("install --path . --example 'ex*1'")
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/example1[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `example1[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn install_bin() {
    full_project()
        .cargo("install --path . --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/bin1[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `bin1[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_example() {
    full_project()
        .cargo("rustdoc -v --example 'ex*1'")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc --edition=2015 --crate-type bin --crate-name example1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/example1/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_bin() {
    full_project()
        .cargo("rustdoc -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc --edition=2015 --crate-type bin --crate-name bin1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/bin1/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_bench() {
    full_project()
        .cargo("rustdoc -v --bench 'be*1'")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc --edition=2015 --crate-type bin --crate-name bench1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/bench1/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_test() {
    full_project()
        .cargo("rustdoc -v --test 'te*1'")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc --edition=2015 --crate-type bin --crate-name test1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/test1/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustc_example() {
    full_project()
        .cargo("rustc -v --example 'ex*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name example1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustc_bin() {
    full_project()
        .cargo("rustc -v --bin 'bi*1'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustc_bench() {
    full_project()
        .cargo("rustc -v --bench 'be*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[RUNNING] `rustc --crate-name bench1 [..]`
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn rustc_test() {
    full_project()
        .cargo("rustc -v --test 'te*1'")
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name bin1 [..]`
[RUNNING] `rustc --crate-name bin2 [..]`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name test1 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

fn full_project() -> Project {
    project()
        .file("examples/example1.rs", "fn main() { }")
        .file("examples/example2.rs", "fn main() { }")
        .file("benches/bench1.rs", "")
        .file("benches/bench2.rs", "")
        .file("tests/test1.rs", "")
        .file("tests/test2.rs", "")
        .file("src/main.rs", "fn main() { }")
        .file("src/bin/bin1.rs", "fn main() { }")
        .file("src/bin/bin2.rs", "fn main() { }")
        .build()
}
