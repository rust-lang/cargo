= cargo-publish(1)
:idprefix: cargo_publish_
:doctype: manpage
:actionverb: Publish

== NAME

cargo-publish - Upload a package to the registry

== SYNOPSIS

`cargo publish [_OPTIONS_]`

== DESCRIPTION

This command will create a distributable, compressed `.crate` file with the
source code of the package in the current directory and upload it to a
registry. The default registry is https://crates.io. This performs the
following steps:

. Performs a few checks, including:
  - Checks the `package.publish` key in the manifest for restrictions on which
    registries you are allowed to publish to.
. Create a `.crate` file by following the steps in man:cargo-package[1].
. Upload the crate to the registry. Note that the server will perform
  additional checks on the crate.

This command requires you to be authenticated with either the `--token` option
or using man:cargo-login[1].

See linkcargo:reference/publishing.html[the reference] for more details about
packaging and publishing.

== OPTIONS

=== Publish Options

*--dry-run*::
  Perform all checks without uploading.

include::options-token.adoc[]

*--no-verify*::
    Don't verify the contents by building them.

*--allow-dirty*::
    Allow working directories with uncommitted VCS changes to be packaged.

include::options-index.adoc[]

include::options-registry.adoc[]

=== Compilation Options

include::options-target-triple.adoc[]

include::options-target-dir.adoc[]

include::options-features.adoc[]

=== Manifest Options

include::options-manifest-path.adoc[]

include::options-locked.adoc[]

=== Miscellaneous Options

include::options-jobs.adoc[]

=== Display Options

include::options-display.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Publish the current package:

    cargo publish

== SEE ALSO
man:cargo[1], man:cargo-package[1], man:cargo-login[1]
