### Package Selection

By default, when no package selection options are given, the packages selected
depend on the selected manifest file (based on the current working directory if
`--manifest-path` is not given). If the manifest is the root of a workspace then
the workspaces default members are selected, otherwise only the package defined
by the manifest will be selected.

The default members of a workspace can be set explicitly with the
`workspace.default-members` key in the root manifest. If this is not set, a
virtual workspace will include all workspace members (equivalent to passing
`--workspace`), and a non-virtual workspace will include only the root crate itself.

{{#options}}

{{#option "`-p` _spec_..." "`--package` _spec_..."}}
{{actionverb}} only the specified packages. See {{man "cargo-pkgid" 1}} for the
SPEC format. This flag may be specified multiple times and supports common Unix
glob patterns like `*`, `?` and `[]`. However, to avoid your shell accidentally 
expanding glob patterns before Cargo handles them, you must use single quotes or
double quotes around each pattern.
{{/option}}

{{#option "`--workspace`" }}
{{actionverb}} all members in the workspace.
{{/option}}

{{#unless noall}}
{{#option "`--all`" }}
Deprecated alias for `--workspace`.
{{/option}}
{{/unless}}

{{#option "`--exclude` _SPEC_..." }}
Exclude the specified packages. Must be used in conjunction with the
`--workspace` flag. This flag may be specified multiple times and supports
common Unix glob patterns like `*`, `?` and `[]`. However, to avoid your shell
accidentally expanding glob patterns before Cargo handles them, you must use
single quotes or double quotes around each pattern.
{{/option}}

{{/options}}
