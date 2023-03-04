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

<dt class="option-term" id="option-options---foo-bar"><a class="option-anchor" href="#option-options---foo-bar"></a><code>--foo-bar</code></dt>
<dd class="option-desc">Demo <em>emphasis</em>, <strong>strong</strong>, <del>strike</del></dd>


<dt class="option-term" id="option-options--p"><a class="option-anchor" href="#option-options--p"></a><code>-p</code> <em>spec</em></dt>
<dt class="option-term" id="option-options---package"><a class="option-anchor" href="#option-options---package"></a><code>--package</code> <em>spec</em></dt>
<dd class="option-desc">This has multiple flags.</dd>


<dt class="option-term" id="option-options-named-arg…"><a class="option-anchor" href="#option-options-named-arg…"></a><em>named-arg…</em></dt>
<dd class="option-desc">A named argument.</dd>


</dl>

### Common Options

<dl>
<dt class="option-term" id="option-options-@filename"><a class="option-anchor" href="#option-options-@filename"></a><code>@</code><em>filename</em></dt>
<dd class="option-desc">Load from filename.</dd>


<dt class="option-term" id="option-options---foo"><a class="option-anchor" href="#option-options---foo"></a><code>--foo</code> [<em>bar</em>]</dt>
<dd class="option-desc">Flag with optional value.</dd>


<dt class="option-term" id="option-options---foo[=bar]"><a class="option-anchor" href="#option-options---foo[=bar]"></a><code>--foo</code>[<code>=</code><em>bar</em>]</dt>
<dd class="option-desc">Alternate syntax for optional value (with required = for disambiguation).</dd>


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
