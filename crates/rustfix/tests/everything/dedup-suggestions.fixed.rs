// See <https://github.com/rust-lang/cargo/issues/13027>
macro_rules! foo {
    () => {
        let x = Box::new(1);
        let _ = &x;
    };
}

fn main() {
    foo!();
    foo!();
}
