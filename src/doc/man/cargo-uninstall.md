= cargo-uninstall(1)
:idprefix: cargo_uninstall_
:doctype: manpage

== NAME

cargo-uninstall - Remove a Rust binary

== SYNOPSIS

`cargo uninstall [_OPTIONS_] [_SPEC_...]`

== DESCRIPTION

This command removes a package installed with man:cargo-install[1]. The _SPEC_
argument is a package ID specification of the package to remove (see
man:cargo-pkgid[1]).

By default all binaries are removed for a crate but the `--bin` and
`--example` flags can be used to only remove particular binaries.

include::description-install-root.adoc[]

== OPTIONS

=== Install Options

*-p*::
*--package* _SPEC_...::
    Package to uninstall.

*--bin* _NAME_...::
    Only uninstall the binary _NAME_.

*--root* _DIR_::
    Directory to uninstall packages from.

=== Display Options

include::options-display.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Uninstall a previously installed package.

    cargo uninstall ripgrep

== SEE ALSO
man:cargo[1], man:cargo-install[1]
