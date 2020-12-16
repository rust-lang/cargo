# links(1)

## NAME

links - Test of different link kinds

## DESCRIPTION

Inline link: [inline link](https://example.com/inline)

Reference link: [this is a link][bar]

Collapsed: [collapsed][]

Shortcut: [shortcut]

Autolink: <https://example.com/auto>

Email: <foo@example.com>

Relative link: [relative link](foo/bar.html)

Collapsed unknown: [collapsed unknown][]

Reference unknown: [foo][unknown]

Shortcut unknown: [shortcut unknown]

{{man "other-cmd" 1}}

{{man "local-cmd" 1}}

{{> links-include}}

## OPTIONS

{{#options}}

{{#option "`--foo-bar`"}}
Example [link](bar.html).
See {{man "other-cmd" 1}}, {{man "local-cmd" 1}}
{{/option}}

{{/options}}


[bar]: https://example.com/bar
[collapsed]: https://example.com/collapsed
[shortcut]: https://example.com/shortcut
