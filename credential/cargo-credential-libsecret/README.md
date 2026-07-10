# cargo-credential-libsecret

This is the implementation for the Cargo credential helper for [GNOME libsecret].
See the [credential-provider] documentation for how to use this.

This credential provider is built-in to cargo as `cargo:libsecret`.

It is available on Unix-like platforms such as Linux and the BSDs — except
macOS, which uses its OS-specific keyring via the `cargo:macos-keychain`
provider instead, and mobile platforms, where libsecret does not exist.

> This crate is maintained by the Cargo team, primarily for use by Cargo
> and not intended for external use (except as a transitive dependency). This
> crate may make major changes to its APIs or be deprecated without warning.

[GNOME libsecret]: https://wiki.gnome.org/Projects/Libsecret
[credential-provider]: https://doc.rust-lang.org/nightly/cargo/reference/registry-authentication.html
