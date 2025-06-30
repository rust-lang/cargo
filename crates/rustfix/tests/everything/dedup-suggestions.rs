// See <https://github.com/rust-lang/cargo/issues/13027>
macro_rules! foo {
    () => {
        let x = Box::new(1);
        std::mem::forget(&x);
    };
}

fn main() {
    foo!();
    foo!();
}
