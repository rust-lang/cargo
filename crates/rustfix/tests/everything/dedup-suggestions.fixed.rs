// See <https://github.com/rust-lang/cargo/issues/13027>
macro_rules! foo {
    () => {
        let _ = &1;
    };
}

fn main() {
    foo!();
    foo!();
}
