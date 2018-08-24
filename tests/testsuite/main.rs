#![deny(warnings)]
#![cfg_attr(feature = "cargo-clippy", allow(blacklisted_name))]
#![cfg_attr(feature = "cargo-clippy", allow(explicit_iter_loop))]

extern crate bufstream;
extern crate cargo;
extern crate filetime;
extern crate flate2;
extern crate git2;
extern crate glob;
extern crate hex;
extern crate libc;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate tar;
extern crate toml;
extern crate url;
#[cfg(windows)]
extern crate winapi;

#[macro_use]
mod support;

mod alt_registry;
mod bad_config;
mod bad_manifest_path;
mod bench;
mod build_auth;
mod build_lib;
mod build;
mod build_plan;
mod build_script_env;
mod build_script;
mod cargo_alias_config;
mod cargo_features;
mod cargo_command;
mod cfg;
mod check;
mod clean;
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
mod jobserver;
mod local_registry;
mod lockfile_compat;
mod login;
mod metabuild;
mod metadata;
mod net_config;
mod new;
mod out_dir;
mod overrides;
mod package;
mod patch;
mod path;
mod plugins;
mod proc_macro;
mod profiles;
mod profile_config;
mod profile_overrides;
mod profile_targets;
mod publish;
mod read_manifest;
mod registry;
mod rename_deps;
mod required_features;
mod resolve;
mod run;
mod rustc;
mod rustc_info_cache;
mod rustdocflags;
mod rustdoc;
mod rustflags;
mod search;
mod shell_quoting;
mod small_fd_limits;
mod test;
mod tool_paths;
mod update;
mod verify_project;
mod version;
mod warn_on_failure;
mod workspaces;
