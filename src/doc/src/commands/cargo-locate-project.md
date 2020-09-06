# cargo-locate-project(1)

## NAME

cargo-locate-project - Print a JSON representation of a Cargo.toml file's location

## SYNOPSIS

`cargo locate-project` [_options_]

## DESCRIPTION

This command will print a JSON object to stdout with the full path to the
`Cargo.toml` manifest.

See also [cargo-metadata(1)](cargo-metadata.md) which is capable of returning the path to a
workspace root.

## OPTIONS

### Formatting Options

<dl>

<dt class="option-term" id="option-cargo-locate-project--f"><a class="option-anchor" href="#option-cargo-locate-project--f"></a><code>-f</code> <em>format</em></dt>
<dt class="option-term" id="option-cargo-locate-project---format"><a class="option-anchor" href="#option-cargo-locate-project---format"></a><code>--format</code> <em>format</em></dt>
<dd class="option-desc">Set the format string for the output representation. The default is a JSON
object holding all of the supported information.</p>
<p>This is an arbitrary string which will be used to display the project location.
The following strings will be replaced with the corresponding value:</p>
<ul>
<li><code>{root}</code> — The absolute path of the <code>Cargo.toml</code> manifest.</li>
</ul>
<p>For example you might use <code>--format '{root}'</code> or more concisely <code>-f{root}</code> to
output just the path and nothing else, or <code>--format 'root={root}'</code> to include a
prefix.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-locate-project--v"><a class="option-anchor" href="#option-cargo-locate-project--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-locate-project---verbose"><a class="option-anchor" href="#option-cargo-locate-project---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for &quot;very verbose&quot; output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-locate-project--q"><a class="option-anchor" href="#option-cargo-locate-project--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-locate-project---quiet"><a class="option-anchor" href="#option-cargo-locate-project---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">No output printed to stdout.</dd>


<dt class="option-term" id="option-cargo-locate-project---color"><a class="option-anchor" href="#option-cargo-locate-project---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>.</dd>


</dl>

### Manifest Options

<dl>
<dt class="option-term" id="option-cargo-locate-project---manifest-path"><a class="option-anchor" href="#option-cargo-locate-project---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-locate-project-+toolchain"><a class="option-anchor" href="#option-cargo-locate-project-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://github.com/rust-lang/rustup/">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-locate-project--h"><a class="option-anchor" href="#option-cargo-locate-project--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-locate-project---help"><a class="option-anchor" href="#option-cargo-locate-project---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-locate-project--Z"><a class="option-anchor" href="#option-cargo-locate-project--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Display the path to the manifest based on the current directory:

       cargo locate-project

## SEE ALSO
[cargo(1)](cargo.md), [cargo-metadata(1)](cargo-metadata.md)
