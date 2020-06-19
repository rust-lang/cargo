use crate::command_prelude::*;

pub fn builtin() -> Vec<App> {
    vec![
        bench::cli(),
        build::cli(),
        check::cli(),
        clean::cli(),
        doc::cli(),
        fetch::cli(),
        #[cfg(feature = "op-fix")]
        fix::cli(),
        generate_lockfile::cli(),
        git_checkout::cli(),
        init::cli(),
        #[cfg(feature = "op-install")]
        install::cli(),
        locate_project::cli(),
        login::cli(),
        metadata::cli(),
        new::cli(),
        owner::cli(),
        #[cfg(feature = "op-package-publish")]
        package::cli(),
        pkgid::cli(),
        #[cfg(feature = "op-package-publish")]
        publish::cli(),
        read_manifest::cli(),
        run::cli(),
        rustc::cli(),
        rustdoc::cli(),
        search::cli(),
        test::cli(),
        tree::cli(),
        uninstall::cli(),
        update::cli(),
        vendor::cli(),
        verify_project::cli(),
        version::cli(),
        yank::cli(),
    ]
}

pub fn builtin_exec(cmd: &str) -> Option<fn(&mut Config, &ArgMatches<'_>) -> CliResult> {
    let f = match cmd {
        "bench" => bench::exec,
        "build" => build::exec,
        "check" => check::exec,
        "clean" => clean::exec,
        "doc" => doc::exec,
        "fetch" => fetch::exec,
        #[cfg(feature = "op-fix")]
        "fix" => fix::exec,
        "generate-lockfile" => generate_lockfile::exec,
        "git-checkout" => git_checkout::exec,
        "init" => init::exec,
        #[cfg(feature = "op-install")]
        "install" => install::exec,
        "locate-project" => locate_project::exec,
        "login" => login::exec,
        "metadata" => metadata::exec,
        "new" => new::exec,
        "owner" => owner::exec,
        #[cfg(feature = "op-package-publish")]
        "package" => package::exec,
        "pkgid" => pkgid::exec,
        #[cfg(feature = "op-package-publish")]
        "publish" => publish::exec,
        "read-manifest" => read_manifest::exec,
        "run" => run::exec,
        "rustc" => rustc::exec,
        "rustdoc" => rustdoc::exec,
        "search" => search::exec,
        "test" => test::exec,
        "tree" => tree::exec,
        "uninstall" => uninstall::exec,
        "update" => update::exec,
        "vendor" => vendor::exec,
        "verify-project" => verify_project::exec,
        "version" => version::exec,
        "yank" => yank::exec,
        _ => return None,
    };
    Some(f)
}

pub mod bench;
pub mod build;
pub mod check;
pub mod clean;
pub mod doc;
pub mod fetch;
#[cfg(feature = "op-fix")]
pub mod fix;
pub mod generate_lockfile;
pub mod git_checkout;
pub mod init;
#[cfg(feature = "op-install")]
pub mod install;
pub mod locate_project;
pub mod login;
pub mod metadata;
pub mod new;
pub mod owner;
#[cfg(feature = "op-package-publish")]
pub mod package;
pub mod pkgid;
#[cfg(feature = "op-package-publish")]
pub mod publish;
pub mod read_manifest;
pub mod run;
pub mod rustc;
pub mod rustdoc;
pub mod search;
pub mod test;
pub mod tree;
pub mod uninstall;
pub mod update;
pub mod vendor;
pub mod verify_project;
pub mod version;
pub mod yank;
