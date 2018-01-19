fn main() {
    let _ = (0..3).fold(false, |acc, x| acc || x > 2);

    let _ = (0..3)
        .fold(false, |acc, x| { acc || x > 2 });

    let _ = (0..3).fold(true, |acc, x| acc && x > 2);
}
