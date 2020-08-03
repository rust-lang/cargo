{{#options}}

{{#option "`--no-run`" }}
Compile, but don't run {{nouns}}.
{{/option}}

{{#option "`--no-fail-fast`" }}
Run all {{nouns}} regardless of failure. Without this flag, Cargo will exit
after the first executable fails. The Rust test harness will run all {{nouns}}
within the executable to completion, this flag only applies to the executable
as a whole.
{{/option}}

{{/options}}
