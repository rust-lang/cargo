use support::{project, execs};
use hamcrest::assert_that;

fn setup() {}

test!(simple {
    let p = project("foo");

    assert_that(p.cargo_process("cargo-version"),
                execs().with_status(0).with_stdout(format!("{}\n",
        env!("CFG_VERSION")).as_slice()));
})
