Editing for the Project
-------------------------------------------
Mutation testing features implemented so far:

- Modular framework: `Mutator` trait with `AddSubMutator` (Add ↔ Sub).
- AST indexing: parses all `.rs` files via Cargo’s file lister; collects `+`/`-` occurrences with stable IDs and source locations (line/column).
- Selective AST cache: caches ASTs only for files with ≥10 add/sub sites to balance memory/time.
- One-at-a-time mutations: flips exactly one operator, runs tests, restores the file, repeats.
- Output modes:
   - Default compact: single-line progress bar with elapsed time, then final metrics only.
   - `--long`: detailed per-mutation logs (start/indexed/results/summary).
- JSON reporting:
   - `--json`: writes `mutation-results.json` (short by default, long with `--long`).
   - Includes per-mutation `{file, id, line, column, outcome}` in long mode.
   - `-d <dir>` writes JSON to a specific directory; always prints the JSON directory at the end.

CLI usage on any Cargo project (external or this repo):

- Compact run: `cargo test --mutation`
- Long run: `cargo test --mutation --long`
- JSON (project root): `cargo test --mutation --json`
- JSON (custom dir): `cargo test --mutation --json -d ".\out"`
-------------------------------------------

Compiling Rust
-------------------------------------------
You will first need to compile the base version of cargo, which you
should have done by now. Next:

1. Enter this command (Without the colon) : cargo --version
   You should see something with an older date code.

2. If you see an older time code skip to step 3. Otherwise,
   Enter this command: $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "User")
   This will set your primary cargo compiler to be the base code

3. Navigate to the folder containing our custom fork of cargo
   Enter this command: cargo build
   This will build the cargo.exe we need

4. Enter this command: $env:PATH = "CHANGETHIS\Cargo-Mutation-Enabled-Toolchain\target\debug;" + $env:PATH
   Please replace CHANGETHIS with whatever the path to the file containing our custom fork is
   This will set the new cargo.exe to be the base compiler

5. Enter this command: cargo --version
   You should see one with a newer or different date code

Every time you want to build cargo, you need to follow these steps. 
Make sure clear your PowerShell session before building as well. 

If you get an access denied error:
You probably forgot to clear your session using the clear command
or
You didn't set the path variable to use the base compiler, see step 2
