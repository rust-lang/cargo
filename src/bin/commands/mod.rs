use command_prelude::*;

pub fn builtin() -> Vec<App> {
    vec![
        bench::cli(),
        build::cli(),
        check::cli(),
        clean::cli(),
        doc::cli(),
        fetch::cli(),
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


mod bench;
mod build;
mod check;
mod clean;
mod doc;
mod fetch;
mod generate_lockfile;
mod git_checkout;
mod init;
mod install;
mod locate_project;
mod login;
mod metadata;
mod new;
mod owner;
mod package;
mod pkgid;
mod publish;
mod read_manifest;
mod run;
mod rustc;
mod rustdoc;
mod search;
mod test;
mod uninstall;
mod update;
mod verify_project;
mod version;
mod yank;
