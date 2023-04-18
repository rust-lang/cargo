//! Helper library for writing Cargo credential processes.
//!
//! A credential process should have a `struct` that implements the `Credential` trait.
//! The `main` function should be called with an instance of that struct, such as:
//!
//! ```rust,ignore
//! fn main() {
//!     cargo_credential::main(MyCredential);
//! }
//! ```
//!
//! This will determine the action to perform (get/store/erase) by looking at
//! the CLI arguments for the first argument that does not start with `-`. It
//! will then call the corresponding method of the trait to perform the
//! requested action.

pub type Error = Box<dyn std::error::Error>;

pub trait Credential {
    /// Returns the name of this credential process.
    fn name(&self) -> &'static str;

    /// Retrieves a token for the given registry.
    fn get(&self, index_url: &str) -> Result<String, Error>;

    /// Stores the given token for the given registry.
    fn store(&self, index_url: &str, token: &str, name: Option<&str>) -> Result<(), Error>;

    /// Removes the token for the given registry.
    ///
    /// If the user is not logged in, this should print a message to stderr if
    /// possible indicating that the user is not currently logged in, and
    /// return `Ok`.
    fn erase(&self, index_url: &str) -> Result<(), Error>;
}

pub struct UnsupportedCredential;

impl Credential for UnsupportedCredential {
    fn name(&self) -> &'static str {
        "unsupported"
    }

    fn get(&self, _index_url: &str) -> Result<String, Error> {
        Err("unsupported".into())
    }

    fn store(&self, _index_url: &str, _token: &str, _name: Option<&str>) -> Result<(), Error> {
        Err("unsupported".into())
    }

    fn erase(&self, _index_url: &str) -> Result<(), Error> {
        Err("unsupported".into())
    }
}

/// Runs the credential interaction by processing the command-line and
/// environment variables.
pub fn main(credential: impl Credential) {
    let name = credential.name();
    if let Err(e) = doit(credential) {
        eprintln!("{} error: {}", name, e);
        std::process::exit(1);
    }
}

fn env(name: &str) -> Result<String, Error> {
    std::env::var(name).map_err(|_| format!("environment variable `{}` is not set", name).into())
}

fn doit(credential: impl Credential) -> Result<(), Error> {
    let which = std::env::args()
        .skip(1)
        .skip_while(|arg| arg.starts_with('-'))
        .next()
        .ok_or_else(|| "first argument must be the {action}")?;
    let index_url = env("CARGO_REGISTRY_INDEX_URL")?;
    let name = std::env::var("CARGO_REGISTRY_NAME_OPT").ok();
    let result = match which.as_ref() {
        "get" => credential.get(&index_url).and_then(|token| {
            println!("{}", token);
            Ok(())
        }),
        "store" => {
            read_token().and_then(|token| credential.store(&index_url, &token, name.as_deref()))
        }
        "erase" => credential.erase(&index_url),
        _ => {
            return Err(format!(
                "unexpected command-line argument `{}`, expected get/store/erase",
                which
            )
            .into())
        }
    };
    result.map_err(|e| format!("failed to `{}` token: {}", which, e).into())
}

fn read_token() -> Result<String, Error> {
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer)?;
    if buffer.ends_with('\n') {
        buffer.pop();
    }
    Ok(buffer)
}
