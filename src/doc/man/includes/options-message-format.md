{{#option "`--message-format` _fmt_" }}
The output format for diagnostic messages. Can be specified multiple times
and consists of comma-separated values. Valid values:

- `human` (default): Display in a human-readable text format. Conflicts with
  `short` and `json`.
- `short`: Emit shorter, human-readable text messages. Conflicts with `human`
  and `json`.
- `json`: Emit JSON messages to stdout. See
  [the reference](../reference/external-tools.html#json-messages)
  for more details. Conflicts with `human` and `short`.
- `json-diagnostic-short`: Ensure the `rendered` field of JSON messages contains
  the "short" rendering from rustc. Cannot be used with `human` or `short`.
- `json-diagnostic-rendered-ansi`: Ensure the `rendered` field of JSON messages
  contains embedded ANSI color codes for respecting rustc's default color
  scheme. Cannot be used with `human` or `short`.
- `json-render-diagnostics`: Instruct Cargo to not include rustc diagnostics
  in JSON messages printed, but instead Cargo itself should render the
  JSON diagnostics coming from rustc. Cargo's own JSON diagnostics and others
  coming from rustc are still emitted. Cannot be used with `human` or `short`.
{{/option}}
