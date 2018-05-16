fn main() {
    // a single character replace, changes x to _x.
    let _x = 42;

    // insert only fix, adds `,` to first match arm
    // why doesnt this replace 1 with 1,?
    match &Some(3) {
        &None => 1,
        &Some(2) => 3,
        _ => 2,
    };
}
