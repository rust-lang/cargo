# Features Examples

The following illustrates some real-world examples of features in action.

## Minimizing build times and file sizes

Some packages use features so that if the features are not enabled, it reduces
the size of the crate and reduces compile time. Some examples are:

* [`syn`] is a popular crate for parsing Rust code. Since it is so popular, it
  is helpful to reduce compile times since it affects so many projects. It has
  a [clearly documented list][syn-features] of features which can be used to
  minimize the amount of code it contains.
* [`regex`] has a [several features][regex-features] that are [well
  documented][regex-docs]. Cutting out Unicode support can reduce the
  resulting file size as it can remove some large tables.
* [`winapi`] has [a large number][winapi-features] of features that
  limit which Windows API bindings it supports.
* [`web-sys`] is another example similar to `winapi` that provides a [huge
  surface area][web-sys-features] of API bindings that are limited by using
  features.

[`winapi`]: https://crates.io/crates/winapi
[winapi-features]: https://github.com/retep998/winapi-rs/blob/0.3.9/Cargo.toml#L25-L431
[`regex`]: https://crates.io/crates/regex
[`syn`]: https://crates.io/crates/syn
[syn-features]: https://docs.rs/syn/1.0.54/syn/#optional-features
[regex-features]: https://github.com/rust-lang/regex/blob/1.4.2/Cargo.toml#L33-L101
[regex-docs]: https://docs.rs/regex/1.4.2/regex/#crate-features
[`web-sys`]: https://crates.io/crates/web-sys
[web-sys-features]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/crates/web-sys/Cargo.toml#L32-L1395

## Extending behavior

The [`serde_json`] package has a [`preserve_order` feature][serde_json-preserve_order]
which [changes the behavior][serde_json-code] of JSON maps to preserve the
order that keys are inserted. Notice that it enables an optional dependency
[`indexmap`] to implement the new behavior.

When changing behavior like this, be careful to make sure the changes are
[SemVer compatible]. That is, enabling the feature should not break code that
usually builds with the feature off.

[`serde_json`]: https://crates.io/crates/serde_json
[serde_json-preserve_order]: https://github.com/serde-rs/json/blob/v1.0.60/Cargo.toml#L53-L56
[SemVer compatible]: features.md#semver-compatibility
[serde_json-code]: https://github.com/serde-rs/json/blob/v1.0.60/src/map.rs#L23-L26
[`indexmap`]: https://crates.io/crates/indexmap

## `no_std` support

Some packages want to support both [`no_std`] and `std` environments. This is
useful for supporting embedded and resource-constrained platforms, but still
allowing extended capabilities for platforms that support the full standard
library.

The [`wasm-bindgen`] package defines a [`std` feature][wasm-bindgen-std] that
is [enabled by default][wasm-bindgen-default]. At the top of the library, it
[unconditionally enables the `no_std` attribute][wasm-bindgen-no_std]. This
ensures that `std` and the [`std` prelude] are not automatically in scope.
Then, in various places in the code ([example1][wasm-bindgen-cfg1],
[example2][wasm-bindgen-cfg2]), it uses `#[cfg(feature = "std")]` attributes
to conditionally enable extra functionality that requires `std`.

[`no_std`]: ../../reference/names/preludes.html#the-no_std-attribute
[`wasm-bindgen`]: https://crates.io/crates/wasm-bindgen
[`std` prelude]: ../../std/prelude/index.html
[wasm-bindgen-std]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/Cargo.toml#L25
[wasm-bindgen-default]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/Cargo.toml#L23
[wasm-bindgen-no_std]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/src/lib.rs#L8
[wasm-bindgen-cfg1]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/src/lib.rs#L270-L273
[wasm-bindgen-cfg2]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/src/lib.rs#L67-L75

## Re-exporting dependency features

It can be convenient to re-export the features from a dependency. This allows
the user depending on the crate to control those features without needing to
specify those dependencies directly. For example, [`regex`] [re-exports the
features][regex-re-export] from the [`regex_syntax`][regex_syntax-features]
package. Users of `regex` don't need to know about the `regex_syntax` package,
but they can still access the features it contains.

[regex-re-export]: https://github.com/rust-lang/regex/blob/1.4.2/Cargo.toml#L65-L89
[regex_syntax-features]: https://github.com/rust-lang/regex/blob/1.4.2/regex-syntax/Cargo.toml#L17-L32

## Vendoring of C libraries

Some packages provide bindings to common C libraries (sometimes referred to as
["sys" crates][sys]). Sometimes these packages give you the choice to use the
C library installed on the system, or to build it from source. For example,
the [`openssl`] package has a [`vendored` feature][openssl-vendored] which
enables the corresponding `vendored` feature of [`openssl-sys`]. The
`openssl-sys` build script has some [conditional logic][openssl-sys-cfg] which
causes it to build from a local copy of the OpenSSL source code instead of
using the version from the system.

The [`curl-sys`] package is another example where the [`static-curl`
feature][curl-sys-static] causes it to build libcurl from source. Notice that
it also has a [`force-system-lib-on-osx`][curl-sys-macos] feature which forces
it [to use the system libcurl][curl-sys-macos-code], overriding the
static-curl setting.

[`openssl`]: https://crates.io/crates/openssl
[`openssl-sys`]: https://crates.io/crates/openssl-sys
[sys]: build-scripts.md#-sys-packages
[openssl-vendored]: https://github.com/sfackler/rust-openssl/blob/openssl-v0.10.31/openssl/Cargo.toml#L19
[build script]: build-scripts.md
[openssl-sys-cfg]: https://github.com/sfackler/rust-openssl/blob/openssl-v0.10.31/openssl-sys/build/main.rs#L47-L54
[`curl-sys`]: https://crates.io/crates/curl-sys
[curl-sys-static]: https://github.com/alexcrichton/curl-rust/blob/0.4.34/curl-sys/Cargo.toml#L49
[curl-sys-macos]: https://github.com/alexcrichton/curl-rust/blob/0.4.34/curl-sys/Cargo.toml#L52
[curl-sys-macos-code]: https://github.com/alexcrichton/curl-rust/blob/0.4.34/curl-sys/build.rs#L15-L20

## Feature precedence

Some packages may have mutually-exclusive features. One option to handle this
is to prefer one feature over another. The [`log`] package is an example. It
has [several features][log-features] for choosing the maximum logging level at
compile-time described [here][log-docs]. It uses [`cfg-if`] to [choose a
precedence][log-cfg-if]. If multiple features are enabled, the higher "max"
levels will be preferred over the lower levels.

[`log`]: https://crates.io/crates/log
[log-features]: https://github.com/rust-lang/log/blob/0.4.11/Cargo.toml#L29-L42
[log-docs]: https://docs.rs/log/0.4.11/log/#compile-time-filters
[log-cfg-if]: https://github.com/rust-lang/log/blob/0.4.11/src/lib.rs#L1422-L1448
[`cfg-if`]: https://crates.io/crates/cfg-if

## Proc-macro companion package

Some packages have a proc-macro that is intimately tied with it. However, not
all users will need to use the proc-macro. By making the proc-macro an
optional-dependency, this allows you to conveniently choose whether or not it
is included. This is helpful, because sometimes the proc-macro version must
stay in sync with the parent package, and you don't want to force the users to
have to specify both dependencies and keep them in sync.

An example is [`serde`] which has a [`derive`][serde-derive] feature which
enables the [`serde_derive`] proc-macro. The `serde_derive` crate is very
tightly tied to `serde`, so it uses an [equals version
requirement][serde-equals] to ensure they stay in sync.

[`serde`]: https://crates.io/crates/serde
[`serde_derive`]: https://crates.io/crates/serde_derive
[serde-derive]: https://github.com/serde-rs/serde/blob/v1.0.118/serde/Cargo.toml#L34-L35
[serde-equals]: https://github.com/serde-rs/serde/blob/v1.0.118/serde/Cargo.toml#L17

## Nightly-only features

Some packages want to experiment with APIs or language features that are only
available on the Rust [nightly channel]. However, they may not want to require
their users to also use the nightly channel. An example is [`wasm-bindgen`]
which has a [`nightly` feature][wasm-bindgen-nightly] which enables an
[extended API][wasm-bindgen-unsize] that uses the [`Unsize`] marker trait that
is only available on the nightly channel at the time of this writing.

Note that at the root of the crate it uses [`cfg_attr` to enable the nightly
feature][wasm-bindgen-cfg_attr]. Keep in mind that the [`feature` attribute]
is unrelated to Cargo features, and is used to opt-in to experimental language
features.

The [`simd_support` feature][rand-simd_support] of the [`rand`] package is another example,
which relies on a dependency that only builds on the nightly channel.

[`wasm-bindgen`]: https://crates.io/crates/wasm-bindgen
[nightly channel]: ../../book/appendix-07-nightly-rust.html
[wasm-bindgen-nightly]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/Cargo.toml#L27
[wasm-bindgen-unsize]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/src/closure.rs#L257-L269
[`Unsize`]: ../../std/marker/trait.Unsize.html
[wasm-bindgen-cfg_attr]: https://github.com/rustwasm/wasm-bindgen/blob/0.2.69/src/lib.rs#L11
[`feature` attribute]: ../../unstable-book/index.html
[`rand`]: https://crates.io/crates/rand
[rand-simd_support]: https://github.com/rust-random/rand/blob/0.7.3/Cargo.toml#L40

## Experimental features

Some packages have new functionality that they may want to experiment with,
without having to commit to the stability of those APIs. The features are
usually documented that they are experimental, and thus may change or break in
the future, even during a minor release. An example is the [`async-std`]
package, which has an [`unstable` feature][async-std-unstable], which [gates
new APIs][async-std-gate] that people can opt-in to using, but may not be
completely ready to be relied upon.

[`async-std`]: https://crates.io/crates/async-std
[async-std-unstable]: https://github.com/async-rs/async-std/blob/v1.8.0/Cargo.toml#L38-L42
[async-std-gate]: https://github.com/async-rs/async-std/blob/v1.8.0/src/macros.rs#L46
