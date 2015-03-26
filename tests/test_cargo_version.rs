use support::{project, execs};
use hamcrest::assert_that;
use cargo;

fn setup() {}

test!(simple {
    let p = project("foo");

    assert_that(p.cargo_process("version"),
                execs().with_status(0).with_stdout(&format!("{}\n",
                                                            cargo::version())));

    assert_that(p.cargo_process("--version"),
                execs().with_status(0).with_stdout(&format!("{}\n",
                                                            cargo::version())));

});
