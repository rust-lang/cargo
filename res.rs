enum MyResult {
    Ok(()),
    Err(())
}

impl MyResult {
    fn wat(self) -> MyResult {
        println!("wat");
        self
    }
}

fn hi2u() -> MyResult {
    Ok(())
}

fn zomg() -> MyResult {
    Ok(try!(hi2u().wat()))
}

fn main() {
    println!("{:?}", zomg());
}
