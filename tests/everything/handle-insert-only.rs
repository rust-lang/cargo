fn main() {
    // insert only fix, adds `,` to first match arm
    // why doesnt this replace 1 with 1,?
    match &Some(3) {
        &None => 1
        &Some(x) => x,
    };
}
