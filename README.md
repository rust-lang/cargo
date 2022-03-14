A convenience wrapper for cargo buildscript input/output.

## Why?

The cargo buildscript API is (necessarily) stringly-typed. This crate helps you
avoid typos, at least in the name of the instruction to cargo or environment
variable that you're reading.

Additionally, [potentially multi-valued variables][irlo] have a subtle footgun,
in that when they're a single value, you might match on `target_family="unix"`,
whereas `target_family="unix,wasm"` is also a valid variable, and would wrongly
be excluded by the naive check. Using this crate forces you to be correct about
multi-valued inputs.

[irlo]: https://internals.rust-lang.org/t/futher-extensions-to-cfg-target-family-and-concerns-about-breakage/16313?u=cad97

## License

This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or distribute this software, either in source code form or as a compiled binary, for any purpose, commercial or non-commercial, and by any means.

In jurisdictions that recognize copyright laws, the author or authors of this software dedicate any and all copyright interest in the software to the public domain. We make this dedication for the benefit of the public at large and to the detriment of our heirs and successors. We intend this dedication to be an overt act of relinquishment in perpetuity of all present and future rights to this software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

If this is insufficient, you may also use this software under any of the following licenses:

- [MIT License](https://spdx.org/licenses/MIT.html)
- [Apache License 2.0](https://spdx.org/licenses/Apache-2.0.html) 
- [BSD Zero Clause License](https://spdx.org/licenses/0BSD.html)
- [Creative Commons Zero v1.0 Universal](https://spdx.org/licenses/CC0-1.0.html)
- [Common Development and Distribution License 1.0](https://spdx.org/licenses/CDDL-1.0.html) 
- [MIT No Attribution](https://spdx.org/licenses/MIT-0.html) 
- [The Unlicense](https://spdx.org/licenses/Unlicense.html) (the above text)
- [Do What The F*ck You Want To Public License](https://spdx.org/licenses/WTFPL.html)

and if for some reason that is insufficient, open a PR (and maybe ask your lawyer some questions about the other libraries you're using).
