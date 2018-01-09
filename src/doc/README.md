# The Cargo Book


### Requirements

Building the book requires [mdBook]. To get it:

[mdBook]: https://github.com/azerupi/mdBook

```console
$ cargo install mdbook
```

### Building

To build the book:

```console
$ mdbook build
```

The output will be in the `book` subdirectory. To check it out, open it in
your web browser.

_Firefox:_
```console
$ firefox book/index.html                       # Linux
$ open -a "Firefox" book/index.html             # OS X
$ Start-Process "firefox.exe" .\book\index.html # Windows (PowerShell)
$ start firefox.exe .\book\index.html           # Windows (Cmd)
```

_Chrome:_
```console
$ google-chrome book/index.html                 # Linux
$ open -a "Google Chrome" book/index.html       # OS X
$ Start-Process "chrome.exe" .\book\index.html  # Windows (PowerShell)
$ start chrome.exe .\book\index.html            # Windows (Cmd)
```


## Contributing

Given that the book is still in a draft state, we'd love your help! Please feel free to open
issues about anything, and send in PRs for things you'd like to fix or change. If your change is
large, please open an issue first, so we can make sure that it's something we'd accept before you
go through the work of getting a PR together.
