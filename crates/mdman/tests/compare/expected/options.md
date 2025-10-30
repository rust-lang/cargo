# my-command(1)

## NAME

my-command - A brief description

## SYNOPSIS

`my-command` [`--abc` | `--xyz`] _name_\
`my-command` [`-f` _file_]\
`my-command` (`-m` | `-M`) [_oldbranch_] _newbranch_\
`my-command` (`-d` | `-D`) [`-r`] _branchname_...

## DESCRIPTION

A description of the command.

* One
    * Sub one
    * Sub two
* Two
* Three


## OPTIONS

### Command options

<dl>

<dt class="option-term" id="option-options---foo-bar"><a class="option-anchor" href="#option-options---foo-bar"><code>--foo-bar</code></a></dt>
<dd class="option-desc"><p>Demo <em>emphasis</em>, <strong>strong</strong>, <del>strike</del></p>
</dd>


<dt class="option-term" id="option-options--p"><a class="option-anchor" href="#option-options--p"><code>-p</code> <em>spec</em></a></dt>
<dt class="option-term" id="option-options---package"><a class="option-anchor" href="#option-options---package"><code>--package</code> <em>spec</em></a></dt>
<dd class="option-desc"><p>This has multiple flags.</p>
</dd>


<dt class="option-term" id="option-options-named-arg…"><a class="option-anchor" href="#option-options-named-arg…"><em>named-arg…</em></a></dt>
<dd class="option-desc"><p>A named argument.</p>
</dd>


<dt class="option-term" id="option-options---complex"><a class="option-anchor" href="#option-options---complex"><code>--complex</code></a></dt>
<dd class="option-desc"><p>This option has a list.</p>
<ul>
<li>alpha</li>
<li>beta</li>
<li>gamma</li>
</ul>
<p>Then text continues here.</p>
</dd>


</dl>

### Common Options

<dl>
<dt class="option-term" id="option-options-@filename"><a class="option-anchor" href="#option-options-@filename"><code>@</code><em>filename</em></a></dt>
<dd class="option-desc"><p>Load from filename.</p>
</dd>


<dt class="option-term" id="option-options---foo"><a class="option-anchor" href="#option-options---foo"><code>--foo</code> [<em>bar</em>]</a></dt>
<dd class="option-desc"><p>Flag with optional value.</p>
</dd>


<dt class="option-term" id="option-options---foo[=bar]"><a class="option-anchor" href="#option-options---foo[=bar]"><code>--foo</code>[<code>=</code><em>bar</em>]</a></dt>
<dd class="option-desc"><p>Alternate syntax for optional value (with required = for disambiguation).</p>
</dd>


<dt class="option-term" id="option-options---split-block"><a class="option-anchor" href="#option-options---split-block"><code>--split-block</code></a></dt>
<dd class="option-desc"><p>An option where the description has a <code>block statement that is split across multiple lines</code></p>
</dd>


</dl>


## EXAMPLES

1. An example

   ```
   my-command --abc
   ```

1. Another example

       my-command --xyz

## SEE ALSO
[other-command(1)](other-command.html) [abc(7)](abc.html)
