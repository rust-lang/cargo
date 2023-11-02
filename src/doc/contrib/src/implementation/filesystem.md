# Filesystem

Cargo tends to get run on a very wide array of file systems. Different file
systems can have a wide range of capabilities, and Cargo should strive to do
its best to handle them. Some examples of issues to deal with:

* Not all file systems support locking. Cargo tries to detect if locking is
  supported, and if not, will ignore lock errors. This isn't ideal, but it is
  difficult to deal with.
* The [`fs::canonicalize`] function doesn't work on all file systems
  (particularly some Windows file systems). If that function is used, there
  should be a fallback if it fails. This function will also return `\\?\`
  style paths on Windows, which can have some issues (such as some tools not
  supporting them, or having issues with relative paths).
* Timestamps can be unreliable. The [`fingerprint`] module has a deeper
  discussion of this. One example is that Docker cache layers will erase the
  fractional part of the time stamp.
* Symlinks are not always supported, particularly on Windows.

[`fingerprint`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/fingerprint/mod.rs
[`fs::canonicalize`]: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
