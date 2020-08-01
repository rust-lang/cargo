*-v*::
*--verbose*::
    Use verbose output. May be specified twice for "very verbose" output which
    includes extra output such as dependency warnings and build script output.
    May also be specified with the `term.verbose`
    linkcargo:reference/config.html[config value].

*-q*::
*--quiet*::
    No output printed to stdout.

*--color* _WHEN_::
    Control when colored output is used. Valid values:
+
- `auto` (default): Automatically detect if color support is available on the
  terminal.
- `always`: Always display colors.
- `never`: Never display colors.

+
May also be specified with the `term.color`
linkcargo:reference/config.html[config value].
