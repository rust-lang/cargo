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

{{{{raw}}}}
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
{{{{/raw}}}}

## OPTIONS

{{#options}}

{{#option "`-t` _type_"}}
Specifies the output type. The following output types are supported:
- `man` — A troff-style man page. Outputs with a numbered extension (like
  `.1`) matching the man page section.
- `md` — A markdown file, after all handlebars processing has been finished.
  Outputs with the `.md` extension.
- `txt` — A text file, rendered for situations where a man page viewer isn't
  available. Outputs with the `.txt` extension.
{{/option}}

{{#option "`-o` _outdir_"}}
Specifies the directory where to save the output.
{{/option}}

{{#option "`--url` _base_url_"}}
Specifies a base URL to use for relative URLs within the document. Any
relative URL will be joined with this URL.
{{/option}}

{{#option "`--man` _name_`:`_section_`=`_url_"}}
Specifies a URL to use for the given man page. When the `\{{man name
section}}` expression is used, the given URL will be inserted as a link. This
may be specified multiple times. If a man page reference does not have a
matching `--man` entry, then a relative link to a file named _name_`.md` will
be used.
{{/option}}

{{#option "_sources..._"}}
The source input filename, may be specified multiple times.
{{/option}}

{{/options}}

## EXAMPLES

1. Convert the given documents to man pages:

       mdman -t man -o doc doc/mdman.md
