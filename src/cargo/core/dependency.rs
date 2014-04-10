
// TODO: add version restrictions
#[deriving(Clone,Eq,Show)]
pub struct Dependency {
  name: ~str,
}

impl Dependency {
  pub fn new(name: &str) -> Dependency {
    Dependency { name: name.to_owned() }
  }

  pub fn get_name<'a>(&'a self) -> &'a str {
    self.name.as_slice()
  }
}
