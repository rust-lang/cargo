{{#options}}
{{#option "`@`_filename_"}}
Load from filename.
{{/option}}

{{#option "`--foo` [_bar_]"}}
Flag with optional value.
{{/option}}

{{#option "`--foo`[`=`_bar_]"}}
Alternate syntax for optional value (with required = for disambiguation).
{{/option}}

{{#option "`--split-block`"}}
An option where the description has a `block statement
that is split across multiple lines`
{{/option}}

{{/options}}
