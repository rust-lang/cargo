*+TOOLCHAIN*::
    If Cargo has been installed with rustup, and the first argument to `cargo`
    begins with `+`, it will be interpreted as a rustup toolchain name (such
    as `+stable` or `+nightly`).
    See the link:https://github.com/rust-lang/rustup/[rustup documentation]
    for more information about how toolchain overrides work.

*-h*::
*--help*::
    Prints help information.

*-Z* _FLAG_...::
    Unstable (nightly-only) flags to Cargo. Run `cargo -Z help` for
    details.
