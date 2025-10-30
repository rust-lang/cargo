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

[other-cmd(1)](https://example.org/commands/other-cmd.html)

[local-cmd(1)](local-cmd.html)

[Some link](foo.html)

<dl>
<dt class="option-term" id="option-links---include"><a class="option-anchor" href="#option-links---include"><code>--include</code></a></dt>
<dd class="option-desc"><p>Testing an <a href="included_link.html">included link</a>.</p>
</dd>

</dl>


## OPTIONS

<dl>

<dt class="option-term" id="option-links---foo-bar"><a class="option-anchor" href="#option-links---foo-bar"><code>--foo-bar</code></a></dt>
<dd class="option-desc"><p>Example <a href="bar.html">link</a>.
See <a href="https://example.org/commands/other-cmd.html">other-cmd(1)</a>, <a href="local-cmd.html">local-cmd(1)</a></p>
</dd>


</dl>


[bar]: https://example.com/bar
[collapsed]: https://example.com/collapsed
[shortcut]: https://example.com/shortcut
