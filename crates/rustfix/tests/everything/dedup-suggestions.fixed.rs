// This fixes rust-lang/rust#123304.
// If that lint stops emitting duplicate suggestions,
// we might need to find a substitution.
#![warn(unsafe_op_in_unsafe_fn)]

macro_rules! foo {
    ($x:ident) => {
        pub unsafe fn $x() { unsafe {
            let _ = String::new().as_mut_vec();
        }}
    };
}

fn main() {
    foo!(a);
    foo!(b);
}
