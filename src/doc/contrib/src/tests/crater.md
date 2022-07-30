# Crater

[Crater](https://github.com/rust-lang/crater) is a tool for compiling and running tests for _every_ crate on [crates.io](https://crates.io) (and a few on GitHub).
It is mainly used for checking the extent of breakage when implementing potentially breaking changes and ensuring lack of breakage by running beta vs stable compiler versions.

Essentially it runs some `cargo` command on every crate twice; once against the "start" toolchain and again against the "end" toolchain.
For example, "start" could be the stable release, and "end" could be beta.
If it passes in "start" but fails with "end", then that is reported as a regression.

There is a bot called [craterbot] which is used to run crater on hardware managed by the rust-lang organization.

Crater is run by the release team during the beta cycle.
If there are any regressions that look like they are caused by Cargo, they should contact the Cargo team to decide how to handle it.

## Running crater

If you have a change that you want to test before the beta release, or you want to test behavior that is not normally exercised by crater, you can do a manual run of crater.
Roughly the steps are:

1. Create a branch with your changes.

   In your clone of cargo, make the changes to incorporate whatever new thing you want to test and push it to a branch on your fork on GitHub.

2. Get a clone of <https://github.com/rust-lang/rust>

3. Create a branch in your rust-lang/rust clone to add your changes.

4. Change the `src/tools/cargo` submodule to point to your new branch.

   Modify `.gitmodules` to point to your clone and branch of cargo with the changes you want to test.
   For example:

   ```bash
   git submodule set-url src/tools/cargo https://github.com/ehuss/cargo.git
   git submodule set-branch --branch my-awesome-feature src/tools/cargo
   git submodule update --remote src/tools/cargo
   git add .gitmodules src/tools/cargo
   git commit
   ```

5. Create an PR on rust-lang/rust.

   Push your submodule changes to GitHub and make a PR.
   Start the PR title with `[EXPERIMENT]` to make it clear what the PR is for and assign yourself or @ghost.

6. Make a "try" build.

   A "try" build creates a full release of x86_64-unknown-linux-gnu and stores it on rust-lang servers.
   This can be done with a comment `@bors try` on the PR (all Cargo team members should have permission to do this).

7. Run crater.

   Look at the [craterbot] docs to determine the command that you want to run.
   There are different modes like `check-only`, `build-and-test`, `rustdoc`, etc.

   You can also choose how many crates to run against.
   If you are uncertain if your cargo changes will work correctly, it might be a good idea to run against `top-100` first to check its behavior.
   This will run much faster.
   You can do a full run afterwards.

   After the try build finishes (which should take a couple hours), ask someone to make a crater run.
   The Cargo team does not have that permission, so just ask someone on Zulip.
   They will need to write a comment to `@craterbot` with the command that you have specified.

8. Wait.

   Crater can take anywhere from a few hours to a few weeks to run depending on how long the [craterbot queue](https://crater.rust-lang.org/) is and which mode you picked and the priority of your job.
   When the crater run finishes, craterbot will post a comment to the PR with a link to a report of the results.

9. Investigate the report.

   Look through the report which contains links to build logs for any regressions or errors.

10. Close the PR.

    Whenever you are done doing crater runs, close your PR.

[craterbot]: https://github.com/rust-lang/crater/blob/master/docs/bot-usage.md


## Advanced crater modes

Crater only has a few built-in modes, such as running `cargo check` or `cargo test`.
You can pass extra flags with `+cargoflags`.

More complex tests can be accomplished by customizing Cargo to perform whatever actions you want.
Since crater essentially runs `cargo check`, you can modify the `check` command to perform whichever actions you want.
For example, to test `cargo fix --edition`, [this commit](https://github.com/ehuss/cargo/commit/6901690a6f8d519efb4fabf48c1c2b94af0c3bd8) intercepted `cargo check` and modified it to instead:

1. Only run on crates with the 2018 edition.
2. Run `cargo fix --edition`.
3. Modify the manifest to switch to the 2021 edition.
4. Run `cargo check` to verify.

If you need to compare the before and after of a command that is not part of crater's built-in modes, that can be more difficult.
Two possible options:

* Work with the infra team to add a new mode.
* Build two custom try builds.
  Each one should modify the `cargo check` command as described above.
  The "start" build should perform whichever action you want with an otherwise unmodified cargo.
  The "end" build should perform whichever action you want with your modified cargo.
  Then, in the `@craterbot` command, specify the start and end hashes of the two try builds.

## Limitations

There are some limitations of crater to consider when running Cargo:

* A crater run without regressions is not a green light to move forward.
   * A large portion of Rust code is not tested, such as closed-source projects or things otherwise not collected by crater.
   * Many crates can't build in crater's environment or are otherwise broken.
   * Some crates have flaky tests.
* Crater runs in an isolated environment.
    * It only runs on Linux x86-64.
    * It does not have network access.
    * The crate source is in a read-only mount.
* Crater does several steps before running the test (using its own copy of the stable toolchain):
    * It generates a lockfile using `generate-lockfile` and includes `-Zno-index-update` to prevent index updates (which makes it run much faster).
    * All dependencies are downloaded ahead-of-time with `cargo fetch`.
* The built-in modes pass several flags to cargo such as `--frozen` or `--message-format=json`.
  It will sometimes use `--all-targets` and sometimes not.
  Check the [crater source](https://github.com/rust-lang/crater/blob/master/src/runner/test.rs) for more details on how it works.
