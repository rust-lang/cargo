## Why Cargo Exists

Cargo is a tool that allows Rust projects to declare their various
dependencies and ensure that you’ll always get a repeatable build.

To accomplish this goal, Cargo does four things:

* Introduces two metadata files with various bits of project information.
* Fetches and builds your project’s dependencies.
* Invokes `rustc` or another build tool with the correct parameters to build
  your project.
* Introduces conventions to make working with Rust projects easier.
