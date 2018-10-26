use command_prelude::*;

pub fn builtin() -> Vec<App> {
    vec![
        bench::cli(),
        build::cli(),
        check::cli(),
        clean::cli(),
        doc::cli(),
        fetch::cli(),
        fix::cli(),
        generate_lockfile::cli(),
        git_checkout::cli(),
        init::cli(),
        install::cli(),
        locate_project::cli(),
        login::cli(),
        metadata::cli(),
        new::cli(),
        owner::cli(),
        package::cli(),
        pkgid::cli(),
        publish::cli(),
        read_manifest::cli(),
        run::cli(),
        rustc::cli(),
        rustdoc::cli(),
        search::cli(),
        test::cli(),
        uninstall::cli(),
        update::cli(),
        verify_project::cli(),
        version::cli(),
        yank::cli(),
    ]
}

pub struct BuiltinExec<'a> {
    pub exec: fn(&'a mut Config, &'a ArgMatches) -> CliResult,
    pub alias_for: Option<&'static str>,
}

pub fn builtin_exec(cmd: &str) -> Option<BuiltinExec> {
    let exec = match cmd {
        "bench" => bench::exec,
        "build" | "b" => build::exec,
        "check" => check::exec,
        "clean" => clean::exec,
        "doc" => doc::exec,
        "fetch" => fetch::exec,
        "fix" => fix::exec,
        "generate-lockfile" => generate_lockfile::exec,
        "git-checkout" => git_checkout::exec,
        "init" => init::exec,
        "install" => install::exec,
        "locate-project" => locate_project::exec,
        "login" => login::exec,
        "metadata" => metadata::exec,
        "new" => new::exec,
        "owner" => owner::exec,
        "package" => package::exec,
        "pkgid" => pkgid::exec,
        "publish" => publish::exec,
        "read-manifest" => read_manifest::exec,
        "run" | "r" => run::exec,
        "rustc" => rustc::exec,
        "rustdoc" => rustdoc::exec,
        "search" => search::exec,
        "test" | "t" => test::exec,
        "uninstall" => uninstall::exec,
        "update" => update::exec,
        "verify-project" => verify_project::exec,
        "version" => version::exec,
        "yank" => yank::exec,
        _ => return None,
    };

    let alias_for = match cmd {
        "b" => Some("build"),
        "r" => Some("run"),
        "t" => Some("test"),
        _ => None,
    };

    Some(BuiltinExec { exec, alias_for })
}

pub mod bench;
pub mod build;
pub mod check;
pub mod clean;
pub mod doc;
pub mod fetch;
pub mod fix;
pub mod generate_lockfile;
pub mod git_checkout;
pub mod init;
pub mod install;
pub mod locate_project;
pub mod login;
pub mod metadata;
pub mod new;
pub mod owner;
pub mod package;
pub mod pkgid;
pub mod publish;
pub mod read_manifest;
pub mod run;
pub mod rustc;
pub mod rustdoc;
pub mod search;
pub mod test;
pub mod uninstall;
pub mod update;
pub mod verify_project;
pub mod version;
pub mod yank;
