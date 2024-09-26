# cargo-credential-1password

A Cargo [credential provider] for [1password].

> This crate is maintained by the Cargo team as a part of an experiment around
> 1password integration. We encourage people to try to use this crate in their projects and
> provide feedback through [issues](https://github.com/rust-lang/cargo/issues/), but do not
> guarantee long term maintenance.

## Usage

`cargo-credential-1password` uses the 1password `op` CLI to store the token. You
must install the `op` CLI from the [1password
website](https://1password.com/downloads/command-line/).

Afterward you need to configure `cargo` to use `cargo-credential-1password` as
the credential provider. You can do this by adding something like the following
to your [cargo config file][credential provider]:

```toml
[registry]
global-credential-providers = ["cargo-credential-1password --account my.1password.com"]
```

Finally, run `cargo login` to save your registry token in 1password.

## CLI Arguments

`cargo-credential-1password` supports the following command-line arguments:

* `--account`: The account name to use. For a list of available accounts, 
  run `op account list`.
* `--vault`: The vault name to use. For a list of available vaults,
  run `op vault list`.

[1password]: https://1password.com/
[credential provider]: https://doc.rust-lang.org/stable/cargo/reference/registry-authentication.html
