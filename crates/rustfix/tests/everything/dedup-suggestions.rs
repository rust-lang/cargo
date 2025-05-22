// See <https://github.com/rust-lang/cargo/issues/13027>
macro_rules! foo {
    () => {
        &1;
    };
}

fn main() {
    foo!();
    foo!();
}
