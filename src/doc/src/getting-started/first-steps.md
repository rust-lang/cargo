cargo new my_app
cd my_app
[
package
]
name = "my_app"
version = "0.1.0"
edition = "2021"

[
dependencies
]
clap = { version = "4.0", features = ["derive"] }  # For easy CLI parsing
use clap::Parser;

/// A simple CLI app built with Rust crates
#[derive(Parser)]
#[command(name = "my_app")]
#[command(about = "A basic app example using clap crate")]
struct Args {
    /// Name to greet (optional)
    #[arg(short, long)]
    name: Option<String>,
}

fn main() {
    let args = Args::parse();

    match args.name {
        Some(name) => println!("Hello, {}! Welcome to your Rust app.", name),
        None => println!("Hello, World! This app uses the clap crate for CLI magic."),
    }
}
Hello, Alice! Welcome to your Rust app.
