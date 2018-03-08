#[allow(unused_variables)]
fn main() {
    let a = (|| 42)();

    let b = (||
        42
    )();

    let c = (|| "x")();
}
