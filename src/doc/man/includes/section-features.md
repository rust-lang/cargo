### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

{{#options}}

{{#option "`-F` _features_" "`--features` _features_" }}
Space or comma separated list of features to activate. Features of workspace
members may be enabled with `package-name/feature-name` syntax. This flag may
be specified multiple times, which enables all specified features.
{{/option}}

{{#option "`--all-features`" }}
Activate all available features of all selected packages.
{{/option}}

{{#option "`--no-default-features`" }}
Do not activate the `default` feature of the selected packages.
{{/option}}

{{/options}}
