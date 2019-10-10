## Build cache

Cargo shares build artifacts among all the packages of a single workspace.
Today, Cargo does not share build results across different workspaces, but 
a similar result can be achieved by using a third party tool, [sccache].

To setup `sccache`, install it with `cargo install sccache` and set 
`RUSTC_WRAPPER` environmental variable to `sccache` before invoking Cargo.
If you use bash, it makes sense to add `export RUSTC_WRAPPER=sccache` to 
`.bashrc`. Alternatively, you can set `build.rustc-wrapper` in the
 [Cargo configuration][config]. Refer to sccache documentation for more
 details.

[sccache]: https://github.com/mozilla/sccache
[config]: ../reference/config.md
