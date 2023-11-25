fn main() {
    let x: &[u8] = b"foo"; //~ ERROR mismatched types
    let y: &[u8; 4] = b"baaa"; //~ ERROR mismatched types
    let z: &str = "foo"; //~ ERROR mismatched types
}
