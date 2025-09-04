//! This file tests that when the commands being run are shown
//! in the output, their arguments are quoted properly
//! so that the command can be run in a terminal.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn features_are_quoted() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = ["mikeyhew@example.com"]
            edition = "2015"

            [features]
            some_feature = []
            default = ["some_feature"]
            "#,
        )
        .file("src/main.rs", "fn main() {error}")
        .build();

    p.cargo("check -v")
        .env("MSYSTEM", "1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..] --cfg 'feature="default"' --cfg 'feature="some_feature"' [..]`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error
  |
  = caused by: process didn't exit successfully: `rustc --crate-name foo --edition=2015 src/main.rs --error-format=json --json=diagnostic-rendered-ansi,artifacts,future-incompat --crate-type bin --emit=dep-info,metadata -C embed-bitcode=no -C debuginfo=2 --cfg 'feature="default"' --cfg 'feature="some_feature"' --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values("default", "some_feature"))' -C metadata=da567554e0b1cd9d -C extra-filename=-90cae25e0ef872b8 --out-dir [ROOT]/foo/target/debug/deps -L dependency=[ROOT]/foo/target/debug/deps` ([EXIT_STATUS]: 1)

"#]])
        .run();
}
