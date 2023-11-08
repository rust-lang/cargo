{{#option "`-v`" "`--verbose`"}}
Use verbose output. May be specified twice for "very verbose" output which
includes extra output such as dependency warnings and build script output.
May also be specified with the `term.verbose`
[config value](../reference/config.html).
{{/option}}

{{#option "`-q`" "`--quiet`"}}
Do not print cargo log messages.
May also be specified with the `term.quiet`
[config value](../reference/config.html).
{{/option}}

{{#option "`--color` _when_"}}
Control when colored output is used. Valid values:

- `auto` (default): Automatically detect if color support is available on the
  terminal.
- `always`: Always display colors.
- `never`: Never display colors.

May also be specified with the `term.color`
[config value](../reference/config.html).
{{/option}}

{{#option "`--warnings`"}}
Overrides how warnings are handled. Valid values:

* `warn` (default): warnings are displayed and do not fail the operation.
* `error`: if any warnings are encountered an error will be emitted at the of the operation.
* `ignore`: warnings will be silently ignored.

May also be specified with the `term.warnings`
[config value](../reference/config.html).
{{/option}}
