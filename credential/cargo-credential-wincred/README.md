# cargo-credential-wincred

This is the implementation for the Cargo credential helper for [Windows Credential Manager].
See the [credential-provider] documentation for how to use this.

This credential provider is built-in to cargo as `cargo:wincred`.

> This crate is maintained by the Cargo team, primarily for use by Cargo
> and not intended for external use (except as a transitive dependency). This
> crate may make major changes to its APIs or be deprecated without warning.

[Windows Credential Manager]: https://support.microsoft.com/en-us/windows/accessing-credential-manager-1b5c916a-6a16-889f-8581-fc16e8165ac0
[credential-provider]: https://doc.rust-lang.org/nightly/cargo/reference/registry-authentication.html
