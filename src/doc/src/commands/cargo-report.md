# cargo-report(1)

## NAME

cargo-report --- Generate and display various kinds of reports

## SYNOPSIS

`cargo report` _type_ [_options_]

### DESCRIPTION

Displays a report of the given _type_ --- currently, only `future-incompat` is supported

## OPTIONS

<dl>

<dt class="option-term" id="option-cargo-report---id"><a class="option-anchor" href="#option-cargo-report---id"></a><code>--id</code> <em>id</em></dt>
<dd class="option-desc">Show the report with the specified Cargo-generated id</dd>


<dt class="option-term" id="option-cargo-report--p"><a class="option-anchor" href="#option-cargo-report--p"></a><code>-p</code> <em>spec</em>…</dt>
<dt class="option-term" id="option-cargo-report---package"><a class="option-anchor" href="#option-cargo-report---package"></a><code>--package</code> <em>spec</em>…</dt>
<dd class="option-desc">Only display a report for the specified package</dd>


</dl>

## EXAMPLES

1. Display the latest future-incompat report:

       cargo report future-incompat

2. Display the latest future-incompat report for a specific package:

       cargo report future-incompat --package my-dep:0.0.1

## SEE ALSO
[Future incompat report](../reference/future-incompat-report.html)

[cargo(1)](cargo.html)
