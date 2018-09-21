## Working on an Existing Cargo Package

If you download an existing package that uses Cargo, it’s really easy
to get going.

First, get the package from somewhere. In this example, we’ll use `rand`
cloned from its repository on GitHub:

```console
$ git clone https://github.com/rust-lang-nursery/rand.git
$ cd rand
```

To build, use `cargo build`:

```console
$ cargo build
   Compiling rand v0.1.0 (file:///path/to/package/rand)
```

This will fetch all of the dependencies and then build them, along with the
package.
