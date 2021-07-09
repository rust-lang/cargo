// See src/cargo/lib.rs for notes on these lint settings.
#![warn(rust_2018_idioms)]
#![allow(clippy::all)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::redundant_clone)]
#![cfg_attr(feature = "deny-warnings", deny(warnings))]

#[macro_use]
extern crate cargo_test_macro;

mod advanced_env;
mod alt_registry;
mod bad_config;
mod bad_manifest_path;
mod bench;
mod build;
mod build_plan;
mod build_script;
mod build_script_env;
mod build_script_extra_link_arg;
mod cache_messages;
mod cargo_alias_config;
mod cargo_command;
mod cargo_config;
mod cargo_env_config;
mod cargo_features;
mod cargo_targets;
mod cfg;
mod check;
mod clean;
mod collisions;
mod concurrent;
mod config;
mod config_cli;
mod config_include;
mod corrupt_git;
mod credential_process;
mod cross_compile;
mod cross_publish;
mod custom_target;
mod death;
mod dep_info;
mod directory;
mod doc;
mod edition;
mod error;
mod features;
mod features2;
mod features_namespaced;
mod fetch;
mod fix;
mod freshness;
mod future_incompat_report;
mod generate_lockfile;
mod git;
mod git_auth;
mod git_gc;
mod glob_targets;
mod help;
mod init;
mod install;
mod install_upgrade;
mod jobserver;
mod list_availables;
mod local_registry;
mod locate_project;
mod lockfile_compat;
mod login;
mod logout;
mod lto;
mod member_discovery;
mod member_errors;
mod message_format;
mod messages;
mod metabuild;
mod metadata;
mod minimal_versions;
mod multitarget;
mod net_config;
mod new;
mod offline;
mod old_cargos;
mod out_dir;
mod owner;
mod package;
mod package_features;
mod patch;
mod path;
mod paths;
mod pkgid;
mod plugins;
mod proc_macro;
mod profile_config;
mod profile_custom;
mod profile_overrides;
mod profile_targets;
mod profiles;
mod progress;
mod pub_priv;
mod publish;
mod publish_lockfile;
mod read_manifest;
mod registry;
mod rename_deps;
mod replace;
mod required_features;
mod run;
mod rust_version;
mod rustc;
mod rustc_info_cache;
mod rustdoc;
mod rustdoc_extern_html;
mod rustdocflags;
mod rustflags;
mod search;
mod shell_quoting;
mod standard_lib;
mod test;
mod timings;
mod tool_paths;
mod tree;
mod tree_graph_features;
mod unit_graph;
mod update;
mod vendor;
mod verify_project;
mod version;
mod warn_on_failure;
mod weak_dep_features;
mod workspaces;
mod yank;

#[cargo_test]
fn aaa_trigger_cross_compile_disabled_check() {
    // This triggers the cross compile disabled check to run ASAP, see #5141
    cargo_test_support::cross_compile::disabled();
}
