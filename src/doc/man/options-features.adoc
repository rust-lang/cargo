=== Feature Selection

The feature flags allow you to control the enabled features for the "current"
package. The "current" package is the package in the current directory, or the
one specified in `--manifest-path`. If running in the root of a virtual
workspace, then the default features are selected for all workspace members,
or all features if `--all-features` is specified.

When no feature options are given, the `default` feature is activated for
every selected package.

*--features* _FEATURES_::
    Space or comma separated list of features to activate. These features only
    apply to the current directory's package. Features of direct dependencies
    may be enabled with `<dep-name>/<feature-name>` syntax. This flag may be
    specified multiple times, which enables all specified features.

*--all-features*::
    Activate all available features of all selected packages.

*--no-default-features*::
    Do not activate the `default` feature of the current directory's
    package.
