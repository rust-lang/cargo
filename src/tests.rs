use std::fs::File;
use std::io::Read;
use serde_json;

fn not_empty(s: &&str) -> bool {
    s.trim().len() > 0
}

#[test]
fn clippy() {
    let mut file = File::open("tests/fixtures/clippy.json").unwrap();
    let mut buffer = String::new();
    file.read_to_string(&mut buffer).unwrap();

    for line in buffer.lines().filter(not_empty) {
        let deserialized: ::diagnostics::Diagnostic = serde_json::from_str(&line).unwrap();
        println!("{:?}", deserialized.message);
    }
}
