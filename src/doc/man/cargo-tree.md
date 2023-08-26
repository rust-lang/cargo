# cargo-tree(1)
{{~*set command="tree"}}
{{~*set actionverb="Display"}}
{{~*set noall=true}}

## NAME

cargo-tree --- Display a tree visualization of a dependency graph

## SYNOPSIS

`cargo tree` [_options_]

## DESCRIPTION

This command will display a tree of dependencies to the terminal. An example
of a simple project that depends on the "rand" package:

```
myproject v0.1.0 (/myproject)
└── rand v0.7.3
    ├── getrandom v0.1.14
    │   ├── cfg-if v0.1.10
    │   └── libc v0.2.68
    ├── libc v0.2.68 (*)
    ├── rand_chacha v0.2.2
    │   ├── ppv-lite86 v0.2.6
    │   └── rand_core v0.5.1
    │       └── getrandom v0.1.14 (*)
    └── rand_core v0.5.1 (*)
[build-dependencies]
└── cc v1.0.50
```

Packages marked with `(*)` have been "de-duplicated". The dependencies for the
package have already been shown elsewhere in the graph, and so are not
repeated. Use the `--no-dedupe` option to repeat the duplicates.

The `-e` flag can be used to select the dependency kinds to display. The
"features" kind changes the output to display the features enabled by
each dependency. For example, `cargo tree -e features`:

```
myproject v0.1.0 (/myproject)
└── log feature "serde"
    └── log v0.4.8
        ├── serde v1.0.106
        └── cfg-if feature "default"
            └── cfg-if v0.1.10
```

In this tree, `myproject` depends on `log` with the `serde` feature. `log` in
turn depends on `cfg-if` with "default" features. When using `-e features` it
can be helpful to use `-i` flag to show how the features flow into a package.
See the examples below for more detail.

### Feature Unification

This command shows a graph much closer to a feature-unified graph Cargo will
build, rather than what you list in `Cargo.toml`. For instance, if you specify
the same dependency in both `[dependencies]` and `[dev-dependencies]` but with
different features on. This command may merge all features and show a `(*)` on
one of the dependency to indicate the duplicate.

As a result, for a mostly equivalent overview of what `cargo build` does,
`cargo tree -e normal,build` is pretty close; for a mostly equivalent overview
of what `cargo test` does, `cargo tree` is pretty close. However, it doesn't
guarantee the exact equivalence to what Cargo is going to build, since a
compilation is complex and depends on lots of different factors.

To learn more about feature unification, check out this
[dedicated section](../reference/features.html#feature-unification).

## OPTIONS

### Tree Options

{{#options}}

{{#option "`-i` _spec_" "`--invert` _spec_" }}
Show the reverse dependencies for the given package. This flag will invert
the tree and display the packages that depend on the given package.

Note that in a workspace, by default it will only display the package's
reverse dependencies inside the tree of the workspace member in the current
directory. The `--workspace` flag can be used to extend it so that it will
show the package's reverse dependencies across the entire workspace. The `-p`
flag can be used to display the package's reverse dependencies only with the
subtree of the package given to `-p`.
{{/option}}

{{#option "`--prune` _spec_" }}
Prune the given package from the display of the dependency tree.
{{/option}}

{{#option "`--depth` _depth_" }}
Maximum display depth of the dependency tree. A depth of 1 displays the direct
dependencies, for example.
{{/option}}

{{#option "`--no-dedupe`" }}
Do not de-duplicate repeated dependencies. Usually, when a package has already
displayed its dependencies, further occurrences will not re-display its
dependencies, and will include a `(*)` to indicate it has already been shown.
This flag will cause those duplicates to be repeated.
{{/option}}

{{#option "`-d`" "`--duplicates`" }}
Show only dependencies which come in multiple versions (implies `--invert`).
When used with the `-p` flag, only shows duplicates within the subtree of the
given package.

It can be beneficial for build times and executable sizes to avoid building
that same package multiple times. This flag can help identify the offending
packages. You can then investigate if the package that depends on the
duplicate with the older version can be updated to the newer version so that
only one instance is built.
{{/option}}

{{#option "`-e` _kinds_" "`--edges` _kinds_" }}
The dependency kinds to display. Takes a comma separated list of values:

- `all` --- Show all edge kinds.
- `normal` --- Show normal dependencies.
- `build` --- Show build dependencies.
- `dev` --- Show development dependencies.
- `features` --- Show features enabled by each dependency. If this is the only
  kind given, then it will automatically include the other dependency kinds.
- `no-normal` --- Do not include normal dependencies.
- `no-build` --- Do not include build dependencies.
- `no-dev` --- Do not include development dependencies.
- `no-proc-macro` --- Do not include procedural macro dependencies.

The `normal`, `build`, `dev`, and `all` dependency kinds cannot be mixed with
`no-normal`, `no-build`, or `no-dev` dependency kinds.

The default is `normal,build,dev`.
{{/option}}

{{#option "`--target` _triple_" }}
Filter dependencies matching the given [target triple](../appendix/glossary.html#target). 
The default is the host platform. Use the value `all` to include *all* targets.
{{/option}}

{{/options}}

### Tree Formatting Options

{{#options}}

{{#option "`--charset` _charset_" }}
Chooses the character set to use for the tree. Valid values are "utf8" or
"ascii". Default is "utf8".
{{/option}}

{{#option "`-f` _format_" "`--format` _format_" }}
Set the format string for each package. The default is "{p}".

This is an arbitrary string which will be used to display each package. The following
strings will be replaced with the corresponding value:

- `{p}` --- The package name.
- `{l}` --- The package license.
- `{r}` --- The package repository URL.
- `{f}` --- Comma-separated list of package features that are enabled.
- `{lib}` --- The name, as used in a `use` statement, of the package's library.
{{/option}}

{{#option "`--prefix` _prefix_" }}
Sets how each line is displayed. The _prefix_ value can be one of:

- `indent` (default) --- Shows each line indented as a tree.
- `depth` --- Show as a list, with the numeric depth printed before each entry.
- `none` --- Show as a flat list.
{{/option}}

{{/options}}

{{> section-package-selection }}

### Manifest Options

{{#options}}

{{> options-manifest-path }}

{{> options-locked }}

{{/options}}

{{> section-features }}

### Display Options

{{#options}}

{{> options-display }}

{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Display the tree for the package in the current directory:

       cargo tree

2. Display all the packages that depend on the `syn` package:

       cargo tree -i syn

3. Show the features enabled on each package:

       cargo tree --format "{p} {f}"

4. Show all packages that are built multiple times. This can happen if multiple
   semver-incompatible versions appear in the tree (like 1.0.0 and 2.0.0).

       cargo tree -d

5. Explain why features are enabled for the `syn` package:

       cargo tree -e features -i syn

   The `-e features` flag is used to show features. The `-i` flag is used to
   invert the graph so that it displays the packages that depend on `syn`. An
   example of what this would display:

   ```
   syn v1.0.17
   ├── syn feature "clone-impls"
   │   └── syn feature "default"
   │       └── rustversion v1.0.2
   │           └── rustversion feature "default"
   │               └── myproject v0.1.0 (/myproject)
   │                   └── myproject feature "default" (command-line)
   ├── syn feature "default" (*)
   ├── syn feature "derive"
   │   └── syn feature "default" (*)
   ├── syn feature "full"
   │   └── rustversion v1.0.2 (*)
   ├── syn feature "parsing"
   │   └── syn feature "default" (*)
   ├── syn feature "printing"
   │   └── syn feature "default" (*)
   ├── syn feature "proc-macro"
   │   └── syn feature "default" (*)
   └── syn feature "quote"
       ├── syn feature "printing" (*)
       └── syn feature "proc-macro" (*)
   ```

   To read this graph, you can follow the chain for each feature from the root
   to see why it is included. For example, the "full" feature is added by the
   `rustversion` crate which is included from `myproject` (with the default
   features), and `myproject` is the package selected on the command-line. All
   of the other `syn` features are added by the "default" feature ("quote" is
   added by "printing" and "proc-macro", both of which are default features).

   If you're having difficulty cross-referencing the de-duplicated `(*)`
   entries, try with the `--no-dedupe` flag to get the full output.

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-metadata" 1}}
