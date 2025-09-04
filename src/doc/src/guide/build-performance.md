# Optimizing Build Performance

Compilation of Rust crates can sometimes be rather slow, due to various reasons (such as the used compilation model and the design of the Rust language and its compiler). There are various approaches that can be used to optimize build performance, which mostly fall under two categories:

- Modify compiler or Cargo flags
- Modify the source code of your crate(s)

This guide focuses on the first approach.

Below, you can find several methods that can be used to optimize build performance. It is important to note that their effect varies a lot based on the compiled crate, and in some cases they can actually make compilation slower. You should always measure build performance on your crate(s) to determine if a given method described here is effective for your crate.

Note that some of these approaches currently require using the nightly toolchain.

## Reduce amount of generated debug information

By default, the `dev` [profile](../reference/profiles.md) enables generation of full debug information (debuginfo) both for local crates and also for all dependencies. This is useful if you want to debug your code with a debugger or profile it with a profiler, but it can also have a significant compilation and linking time cost.

You can reduce that cost by reducing the amount of debuginfo that is generated. The fastest option is `debug = false`, which completely turns off debuginfo generation, but a reasonable trade-off could also be setting `debug = "line-tables-only"`, which only generates enough debuginfo to support proper source code links in backtraces, which are generated e.g. when a panic happens.

Here is an example of configuring debuginfo generation in `Cargo.toml`:
```toml
[profile.dev]
debug = false # or "line-tables-only"
```

If you want to keep debuginfo for your crate only, but you do not need it for your dependencies, you can set `debug = false` as the default value for a given profile, and then enable debuginfo only for your crate:

```toml
[profile.dev]
debug = false

[profile.dev.package]
<your-crate-name>.debug = true
```

## Use an alternative codegen backend

> **This requires nightly/unstable features**

The component of the Rust compiler that generates executable code is called a "codegen backend". The default backend is LLVM, which produces very optimized code, at the cost of relatively slow compilation time. You can try to use a different codegen backend in order to speed up the compilation of your crate.

You can use the [Cranelift](https://github.com/rust-lang/rustc_codegen_cranelift) backend, which is designed for fast(er) compilation time. You can install this backend using rustup:

```console
$ rustup component add rustc-codegen-cranelift-preview --toolchain nightly
```

and then enable it for a given Cargo profile using the `codegen-backend` option in `Cargo.toml`:
```toml
[profile.dev]
codegen-backend = "cranelift"
```

Since this is currently an unstable option, you will also need to either pass `-Z codegen-backend` to Cargo, or enable this unstable option in the `.cargo/config.toml` file. You can find more information about the unstable `codegen-backend` profile option [here](../reference/unstable.md#codegen-backend).

Note that the Cranelift backend might not support all features used by your crate. It is also available only for a limited set of targets.
