## Package Layout

Cargo uses conventions for file placement to make it easy to dive into a new
Cargo package:

```
.
├── Cargo.lock
├── Cargo.toml
├── benches
│   └── large-input.rs
├── examples
│   └── simple.rs
├── src
│   ├── bin
│   │   └── another_executable.rs
│   ├── lib.rs
│   └── main.rs
└── tests
    └── some-integration-tests.rs
```

* `Cargo.toml` and `Cargo.lock` are stored in the root of your package (*package
  root*).
* Source code goes in the `src` directory.
* The default library file is `src/lib.rs`.
* The default executable file is `src/main.rs`.
* Other executables can be placed in `src/bin/*.rs`.
* Integration tests go in the `tests` directory (unit tests go in each file
  they're testing).
* Examples go in the `examples` directory.
* Benchmarks go in the `benches` directory.

These are explained in more detail in the [manifest
description](../reference/manifest.md#the-project-layout).
