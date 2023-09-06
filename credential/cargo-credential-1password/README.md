# cargo-credential-1password

A Cargo [credential provider] for [1password].

`cargo-credential-1password` uses the 1password `op` CLI to store the token. You must
install the `op` CLI from the [1password
website](https://1password.com/downloads/command-line/). You must run `op signin`
at least once with the appropriate arguments (such as `op signin my.1password.com user@example.com`),
unless you provide the sign-in-address and email arguments. The master password will be required on each request
unless the appropriate `OP_SESSION` environment variable is set. It supports
the following command-line arguments:
* `--account`: The account shorthand name to use.
* `--vault`: The vault name to use.
* `--sign-in-address`: The sign-in-address, which is a web address such as `my.1password.com`.
* `--email`: The email address to sign in with.

[1password]: https://1password.com/
[credential provider]: https://doc.rust-lang.org/nightly/cargo/reference/registry-authentication.html
