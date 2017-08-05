## Working on an existing Cargo project

If you download an existing project that uses Cargo, it’s really easy
to get going.

First, get the project from somewhere. In this example, we’ll use `rand`
cloned from its repository on GitHub:

```shell
$ git clone https://github.com/rust-lang-nursery/rand.git
$ cd rand
```

To build, use `cargo build`:

```shell
$ cargo build
   Compiling rand v0.1.0 (file:///path/to/project/rand)
```

This will fetch all of the dependencies and then build them, along with the
project.
