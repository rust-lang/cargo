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
