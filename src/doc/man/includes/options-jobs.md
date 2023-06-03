{{#option "`-j` _N_" "`--jobs` _N_"}}
Number of parallel jobs to run. May also be specified with the
`build.jobs` [config value](../reference/config.html). Defaults to
the number of logical CPUs. If negative, it sets the maximum number of
parallel jobs to the number of logical CPUs plus provided value. If
a string `default` is provided, it sets the value back to defaults.
Should not be 0.
{{/option}}
