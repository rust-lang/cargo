// Point at the captured immutable outer variable

// Suppress unrelated warnings
#![allow(unused)]

fn foo(mut f: Box<dyn FnMut()>) {
    f();
}

fn main() {
    let y = true;
    foo(Box::new(move || y = false) as Box<_>); //~ ERROR cannot assign to captured outer variable
}
