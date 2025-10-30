# mdman(1)

## NAME

mdman - Converts markdown to a man page

## SYNOPSIS

`mdman` [_options_] `-t` _type_ `-o` _outdir_ _sources..._

## DESCRIPTION

Converts a markdown file to a man page.

The source file is first processed as a
[handlebars](https://handlebarsjs.com/) template. Then, it is processed as
markdown into the target format. This supports different output formats,
such as troff or plain text.

Every man page should start with a level-1 header with the man name and
section, such as `# mdman(1)`.

The handlebars template has several special tags to assist with generating the
man page:

- Every block of command-line options must be wrapped between `{{#options}}`
  and `{{/options}}` tags. This tells the processor where the options start
  and end.
- Each option must be expressed with a `{{#option}}` block. The parameters to
  the block are a sequence of strings indicating the option. For example,
  ```{{#option "`-p` _spec_..." "`--package` _spec_..."}}``` is an option that
  has two different forms. The text within the string is processed as markdown.
  It is recommended to use formatting similar to this example.

  The content of the `{{#option}}` block should contain a detailed description
  of the option.

  Use the `{{/option}}` tag to end the option block.
- References to other man pages should use the `{{man name section}}`
  expression. For example, `{{man "mdman" 1}}` will generate a reference to
  the `mdman(1)` man page. For non-troff output, the `--man` option will tell
  `mdman` how to create links to the man page. If there is no matching `--man`
  option, then it links to a file named _name_`.md` in the same directory.
- Variables can be set with `{{*set name="value"}}`. These variables can
  then be referenced with `{{name}}` expressions.
- Partial templates should be placed in a directory named `includes`
  next to the source file. Templates can be included with an expression like
  `{{> template-name}}`.
- Other helpers include:
    - `{{lower value}}` Converts the given value to lowercase.

## OPTIONS

<dl>

<dt class="option-term" id="option-mdman--t"><a class="option-anchor" href="#option-mdman--t"><code>-t</code> <em>type</em></a></dt>
<dd class="option-desc"><p>Specifies the output type. The following output types are supported:</p>
<ul>
<li><code>man</code> — A troff-style man page. Outputs with a numbered extension (like
<code>.1</code>) matching the man page section.</li>
<li><code>md</code> — A markdown file, after all handlebars processing has been finished.
Outputs with the <code>.md</code> extension.</li>
<li><code>txt</code> — A text file, rendered for situations where a man page viewer isn’t
available. Outputs with the <code>.txt</code> extension.</li>
</ul>
</dd>


<dt class="option-term" id="option-mdman--o"><a class="option-anchor" href="#option-mdman--o"><code>-o</code> <em>outdir</em></a></dt>
<dd class="option-desc"><p>Specifies the directory where to save the output.</p>
</dd>


<dt class="option-term" id="option-mdman---url"><a class="option-anchor" href="#option-mdman---url"><code>--url</code> <em>base_url</em></a></dt>
<dd class="option-desc"><p>Specifies a base URL to use for relative URLs within the document. Any
relative URL will be joined with this URL.</p>
</dd>


<dt class="option-term" id="option-mdman---man"><a class="option-anchor" href="#option-mdman---man"><code>--man</code> <em>name</em><code>:</code><em>section</em><code>=</code><em>url</em></a></dt>
<dd class="option-desc"><p>Specifies a URL to use for the given man page. When the <code>{{man name section}}</code> expression is used, the given URL will be inserted as a link. This
may be specified multiple times. If a man page reference does not have a
matching <code>--man</code> entry, then a relative link to a file named <em>name</em><code>.md</code> will
be used.</p>
</dd>


<dt class="option-term" id="option-mdman-sources…"><a class="option-anchor" href="#option-mdman-sources…"><em>sources…</em></a></dt>
<dd class="option-desc"><p>The source input filename, may be specified multiple times.</p>
</dd>


</dl>

## EXAMPLES

1. Convert the given documents to man pages:

       mdman -t man -o doc doc/mdman.md
