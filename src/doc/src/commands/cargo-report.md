# cargo-report(1)

## NAME

cargo-report --- Generate and display various kinds of reports

## SYNOPSIS

`cargo report` _type_ [_options_]

### DESCRIPTION

Displays a report of the given _type_ --- currently, only `future-incompat` is supported

## OPTIONS

<dl>

<dt class="option-term" id="option-cargo-report---id"><a class="option-anchor" href="#option-cargo-report---id"><code>--id</code> <em>id</em></a></dt>
<dd class="option-desc"><p>Show the report with the specified Cargo-generated id</p>
</dd>


<dt class="option-term" id="option-cargo-report--p"><a class="option-anchor" href="#option-cargo-report--p"><code>-p</code> <em>spec</em>…</a></dt>
<dt class="option-term" id="option-cargo-report---package"><a class="option-anchor" href="#option-cargo-report---package"><code>--package</code> <em>spec</em>…</a></dt>
<dd class="option-desc"><p>Only display a report for the specified package</p>
</dd>


</dl>

## EXAMPLES

1. Display the latest future-incompat report:

       cargo report future-incompat

2. Display the latest future-incompat report for a specific package:

       cargo report future-incompat --package my-dep:0.0.1

## SEE ALSO
[Future incompat report](../reference/future-incompat-report.html)

[cargo(1)](cargo.html)
