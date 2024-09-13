#![cfg(unix)]

use cargo_test_support::cargo_test;
use completest_pty::Runtime;
use snapbox::assert_data_eq;

#[cargo_test(requires_bash)]
#[cfg_attr(target_os = "macos", ignore = "bash is not working on macOS")]
fn bash() {
    let input = "cargo \t\t";
    let expected = snapbox::str![
        "% 
--version          --help             check              install            read-manifest      update
--list             -V                 clean              locate-project     remove             vendor
--explain          -v                 config             login              report             verify-project
--verbose          -q                 doc                logout             run                version
--quiet            -C                 fetch              metadata           rustc              yank
--color            -Z                 fix                new                rustdoc            
--locked           -h                 generate-lockfile  owner              search             
--offline          add                help               package            test               
--frozen           bench              info               pkgid              tree               
--config           build              init               publish            uninstall          "
    ];
    let actual = complete(input, "bash");
    assert_data_eq!(actual, expected);
}

#[cargo_test(requires_elvish)]
#[cfg_attr(target_os = "macos", ignore = "elvish is not working on macOS")]
fn elvish() {
    let input = "cargo \t\t";
    let expected = snapbox::str![
        "% cargo --config
 COMPLETING argument  
--color    --version  check              install         read-manifest  update        
--config   -C         clean              locate-project  remove         vendor        
--explain  -V         config             login           report         verify-project
--frozen   -Z         doc                logout          run            version       
--help     -h         fetch              metadata        rustc          yank          
--list     -q         fix                new             rustdoc      
--locked   -v         generate-lockfile  owner           search       
--offline  add        help               package         test         
--quiet    bench      info               pkgid           tree         
--verbose  build      init               publish         uninstall    "
    ];
    let actual = complete(input, "elvish");
    assert_data_eq!(actual, expected);
}

#[cargo_test(requires_fish)]
#[cfg_attr(target_os = "macos", ignore = "fish is not working on macOS")]
fn fish() {
    let input = "cargo \t\t";
    let expected = snapbox::str![
        "% cargo 
--version                                                                                  (Print version info and exit)
--list                                                                                         (List installed commands)
--explain                                                      (Provide a detailed explanation of a rustc error message)
--verbose                                                        (Use verbose output (-vv very verbose/build.rs output))
--quiet                                                                                (Do not print cargo log messages)
--color                                                                                  (Coloring: auto, always, never)
--locked                                                                (Assert that `Cargo.lock` will remain unchanged)
--offline                                                                            (Run without accessing the network)
--frozen                                                          (Equivalent to specifying both --locked and --offline)
--config                                                                                (Override a configuration value)
--help                                                                                                      (Print help)
-V                                                                                         (Print version info and exit)
-v                                                               (Use verbose output (-vv very verbose/build.rs output))
-q                                                                                     (Do not print cargo log messages)
-C                                                            (Change to DIRECTORY before doing anything (nightly-only))
-Z                                             (Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details)
-h                                                                                                          (Print help)
add                                                                     (Add dependencies to a Cargo.toml manifest file)
bench                                                                        (Execute all benchmarks of a local package)
build                                                              (Compile a local package and all of its dependencies)
check                                                     (Check a local package and all of its dependencies for errors)
clean                                                            (Remove artifacts that cargo has generated in the past)
config                                                                                    (Inspect configuration values)
doc                                                                                    (Build a package's documentation)
fetch                                                                 (Fetch dependencies of a package from the network)
fix                                                                  (Automatically fix lint warnings reported by rustc)
generate-lockfile                                                                  (Generate the lockfile for a package)
help                                                                              (Displays help for a cargo subcommand)
info                                                               (Display information about a package in the registry)
init                                                               (Create a new cargo package in an existing directory)
install                                                                                          (Install a Rust binary)
locate-project                                             (Print a JSON representation of a Cargo.toml file's location)
login                                                                                            (Log in to a registry.)
logout                                                                   (Remove an API token from the registry locally)
metadata  (Output the resolved dependencies of a package, the concrete used versions including overrides, in machine-r…)
new                                                                               (Create a new cargo package at <path>)
owner                                                                     (Manage the owners of a crate on the registry)
package                                                        (Assemble the local package into a distributable tarball)
pkgid                                                                    (Print a fully qualified package specification)
publish                                                                               (Upload a package to the registry)
read-manifest                                                    (Print a JSON representation of a Cargo.toml manifest.)
remove                                                             (Remove dependencies from a Cargo.toml manifest file)
report                                                                   (Generate and display various kinds of reports)
run                                                                       (Run a binary or example of the local package)
rustc                                                        (Compile a package, and pass extra options to the compiler)
rustdoc                                                 (Build a package's documentation, using specified custom flags.)
search                                                  (Search packages in the registry. Default registry is crates.io)
test                                      (Execute all unit and integration tests and build examples of a local package)
tree                                                                (Display a tree visualization of a dependency graph)
uninstall                                                                                         (Remove a Rust binary)
update                                                          (Update dependencies as recorded in the local lock file)
vendor                                                                   (Vendor all dependencies for a project locally)
verify-project                                                                     (Check correctness of crate manifest)
version                                                                                       (Show version information)
yank                                                                              (Remove a pushed crate from the index)"];

    let actual = complete(input, "fish");
    assert_data_eq!(actual, expected);
}

#[cargo_test(requires_zsh)]
fn zsh() {
    let input = "cargo \t\t";
    let expected = snapbox::str![
        "% cargo
--color            --version          check              install            read-manifest      update
--config           -C                 clean              locate-project     remove             vendor
--explain          -V                 config             login              report             verify-project
--frozen           -Z                 doc                logout             run                version
--help             -h                 fetch              metadata           rustc              yank
--list             -q                 fix                new                rustdoc            
--locked           -v                 generate-lockfile  owner              search             
--offline          add                help               package            test               
--quiet            bench              info               pkgid              tree               
--verbose          build              init               publish            uninstall          "
    ];
    let actual = complete(input, "zsh");
    assert_data_eq!(actual, expected);
}

fn complete(input: &str, shell: &str) -> String {
    let shell = shell.into();

    // Load the runtime
    let mut runtime = match shell {
        "bash" => load_runtime::<completest_pty::BashRuntimeBuilder>("bash"),
        "elvish" => load_runtime::<completest_pty::ElvishRuntimeBuilder>("elvish"),
        "fish" => load_runtime::<completest_pty::FishRuntimeBuilder>("fish"),
        "zsh" => load_runtime::<completest_pty::ZshRuntimeBuilder>("zsh"),
        _ => panic!("Unsupported shell: {}", shell),
    };

    // Exec the completion
    let term = completest_pty::Term::new();
    let actual = runtime.complete(input, &term).unwrap();

    actual
}

// Return the scratch directory to keep it not being dropped
fn load_runtime<R: completest_pty::RuntimeBuilder>(shell: &str) -> Box<dyn completest_pty::Runtime>
where
    <R as completest_pty::RuntimeBuilder>::Runtime: 'static,
{
    let home = cargo_test_support::paths::home();

    let bin_path = cargo_test_support::cargo_exe();
    let bin_root = bin_path.parent().unwrap().to_owned();

    let mut runtime = Box::new(R::new(bin_root, home).unwrap());

    match shell {
        "bash" => runtime
            .register("", "source <(CARGO_COMPLETE=bash cargo)")
            .unwrap(),
        "elvish" => runtime
            .register("", "eval (E:CARGO_COMPLETE=elvish cargo | slurp)")
            .unwrap(),
        "fish" => runtime
            .register("cargo", "source (CARGO_COMPLETE=fish cargo | psub)")
            .unwrap(),
        "zsh" => runtime
            .register(
                "cargo",
                "#compdef cargo
source <(CARGO_COMPLETE=zsh cargo)",
            )
            .unwrap(),
        _ => panic!("Unsupported shell: {}", shell),
    }

    runtime
}
