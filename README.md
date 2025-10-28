Editing for the Project
-------------------------------------------
As far as I know we should only be working with (Or at least currently)
cargo_test.rs in Cargo-Mutation-Enabled-Toolchain\src\cargo\ops
mutation_iabr.rs in Cargo-Mutation-Enabled-Toolchain\src\cargo\ops
test.rs in Cargo-Mutation-Enabled-Toolchain\src\bin\cargo\commands
-------------------------------------------

Compiling Rust
-------------------------------------------
You will first need to compile the base version of cargo, which you
should have done by now. Next:

1. Enter this command (Without the colon) : cargo --version
   You should see something with an older date code.

2. If you see an older time code skip to step 3. Otherwise,
   Enter this command: $env:PATH = "C:\Users\USER\.cargo\bin;" + $env:PATH
   Please replace C: with your primary drive
   Please replace USER with the name of your local user account
   This will set your primary cargo compiler to be the base fork

3. Navigate to the folder containing our custom fork of cargo
   Enter this command: cargo build
   This will build the cargo.exe we need

4. Enter this command: $env:PATH = "PATH\Cargo-Mutation-Enabled-Toolchain\target\debug;" + $env:PATH
   Please replace PATH with whatever the path to the file containing our custom fork is
   This will set the new cargo.exe to be the base compiler

5. Enter this command: cargo --version
   You should see one with a newer or different date code

Every time you want to build cargo, you need to follow these steps. 
Make sure clear your PowerShell session before building as well. 

If you get an access denied error:
You probably forgot to clear your session using the clear command
or
You didn't set the path variable to use the base compiler, see step 2