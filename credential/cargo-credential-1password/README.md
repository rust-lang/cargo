# cargo-credential-1password

A Cargo [credential provider] for [1password].

`cargo-credential-1password` uses the 1password `op` CLI to store the token. You
must install the `op` CLI from the [1password
website](https://1password.com/downloads/command-line/).

`cargo-credential-1password` supports the following command-line arguments:

* `--account`: The account name to use. For a list of available accounts, 
  run `op account list`.
* `--vault`: The vault name to use. For a list of available vaults,
  run `op vault list`.

[1password]: https://1password.com/
[credential provider]: https://doc.rust-lang.org/stable/cargo/reference/registry-authentication.html
