#![warn(rust_2018_idioms)] // while we're getting used to 2018
#![cfg_attr(feature = "deny-warnings", deny(warnings))]
#![allow(clippy::blacklisted_name)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::redundant_closure)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::redundant_clone)]

#[macro_use]
extern crate cargo_test_macro;

#[macro_use]
mod support;

mod alt_registry;
mod bad_config;
mod bad_manifest_path;
mod bench;
mod build;
mod build_auth;
mod build_lib;
mod build_plan;
mod build_script;
mod build_script_env;
mod cache_messages;
mod cargo_alias_config;
mod cargo_command;
mod cargo_features;
mod cfg;
mod check;
mod clean;
mod collisions;
mod concurrent;
mod config;
mod corrupt_git;
mod cross_compile;
mod cross_publish;
mod custom_target;
mod death;
mod dep_info;
mod directory;
mod doc;
mod edition;
mod features;
mod fetch;
mod fix;
mod freshness;
mod generate_lockfile;
mod git;
mod init;
mod install;
mod install_upgrade;
mod jobserver;
mod list_targets;
mod local_registry;
mod lockfile_compat;
mod login;
mod member_errors;
mod metabuild;
mod metadata;
mod net_config;
mod new;
mod offline;
mod out_dir;
mod overrides;
mod package;
mod patch;
mod path;
mod plugins;
mod proc_macro;
mod profile_config;
mod profile_overrides;
mod profile_targets;
mod profiles;
mod pub_priv;
mod publish;
mod publish_lockfile;
mod read_manifest;
mod registry;
mod rename_deps;
mod required_features;
mod resolve;
mod run;
mod rustc;
mod rustc_info_cache;
mod rustdoc;
mod rustdocflags;
mod rustflags;
mod search;
mod shell_quoting;
mod small_fd_limits;
mod test;
mod tool_paths;
mod update;
mod vendor;
mod verify_project;
mod version;
mod warn_on_failure;
mod workspaces;

#[cargo_test]
fn aaa_trigger_cross_compile_disabled_check() {
    // This triggers the cross compile disabled check to run ASAP, see #5141
    support::cross_compile::disabled();
}
