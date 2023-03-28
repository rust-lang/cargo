//! Tests for the `cargo init` command.

mod auto_git;
mod bin_already_exists_explicit;
mod bin_already_exists_explicit_nosrc;
mod bin_already_exists_implicit;
mod bin_already_exists_implicit_namenosrc;
mod bin_already_exists_implicit_namesrc;
mod bin_already_exists_implicit_nosrc;
mod both_lib_and_bin;
mod cant_create_library_when_both_binlib_present;
mod confused_by_multiple_lib_files;
mod creates_binary_when_both_binlib_present;
mod creates_binary_when_instructed_and_has_lib_file;
mod creates_library_when_instructed_and_has_bin_file;
mod explicit_bin_with_git;
mod formats_source;
mod fossil_autodetect;
mod git_autodetect;
mod git_ignore_exists_no_conflicting_entries;
mod help;
mod ignores_failure_to_format_source;
mod inferred_bin_with_git;
mod inferred_lib_with_git;
mod inherit_workspace_package_table;
mod invalid_dir_name;
mod lib_already_exists_nosrc;
mod lib_already_exists_src;
mod mercurial_autodetect;
mod multibin_project_name_clash;
#[cfg(not(windows))]
mod no_filename;
#[cfg(unix)]
mod path_contains_separator;
mod pijul_autodetect;
mod reserved_name;
mod simple_bin;
mod simple_git;
mod simple_git_ignore_exists;
mod simple_hg;
mod simple_hg_ignore_exists;
mod simple_lib;
mod unknown_flags;
mod with_argument;
