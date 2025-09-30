---
edition = "2015"

[dependencies]
docopt = "0.6"
rustc-serialize = "0.4"
semver = "0.1"
toml = "0.1"
clippy = "0.4"

[dev-dependencies]
regex = "0.1.1"
serde = "1.0.90"

[features]
std = ["serde/std", "semver/std"]
---

fn main() {
}
