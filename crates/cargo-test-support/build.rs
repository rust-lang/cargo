#![allow(clippy::disallowed_methods)]

use build_rs::{input, output};

fn main() {
    output::rustc_check_cfgs(&["emulate_second_only_system"]);
    output::rustc_env("NATIVE_ARCH", &input::target());
    output::rerun_if_changed("build.rs");
}
