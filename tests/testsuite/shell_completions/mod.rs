use std::path::PathBuf;

use cargo::util::command_prelude::*;
use cargo_test_support::cargo_test;

#[cargo_test]
fn test_get_bin_candidates() {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let cwd = PathBuf::from(file!()).parent().unwrap().join("template");
    let cwd = current_dir.join(cwd);

    let expected = snapbox::str![
        "bench_crate_1
bench_crate_2
template"
    ];
    let actual = print_candidates(get_bin_candidates(Some(cwd)));
    snapbox::assert_data_eq!(actual, expected);
}

#[cargo_test]
fn test_get_bench_candidates() {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let cwd = PathBuf::from(file!()).parent().unwrap().join("template");
    let cwd = current_dir.join(cwd);

    let expected = snapbox::str![
        "bench1
bench2"
    ];
    let actual = print_candidates(get_bench_candidates(Some(cwd)));
    snapbox::assert_data_eq!(actual, expected);
}

#[cargo_test]
fn test_get_test_candidates() {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let cwd = PathBuf::from(file!()).parent().unwrap().join("template");
    let cwd = current_dir.join(cwd);

    let expected = snapbox::str![
        "test1
test2"
    ];
    let actual = print_candidates(get_test_candidates(Some(cwd)));
    snapbox::assert_data_eq!(actual, expected);
}

#[cargo_test]
fn test_get_example_candidates() {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let cwd = PathBuf::from(file!()).parent().unwrap().join("template");
    let cwd = current_dir.join(cwd);

    let expected = snapbox::str![
        "example1
example2"
    ];
    let actual = print_candidates(get_example_candidates(Some(cwd)));
    snapbox::assert_data_eq!(actual, expected);
}

#[cargo_test]
fn test_get_registry_candidates() {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let cwd = PathBuf::from(file!()).parent().unwrap().join("template");
    let cwd = current_dir.join(cwd);

    let expected = snapbox::str![
        "my-registry1
my-registry2"
    ];
    let actual = print_candidates(get_registry_candidates(Some(cwd)).unwrap());
    snapbox::assert_data_eq!(actual, expected);
}

fn print_candidates(candidates: Vec<clap_complete::CompletionCandidate>) -> String {
    candidates
        .into_iter()
        .map(|candidate| {
            let compl = candidate.get_value().to_str().unwrap();
            if let Some(help) = candidate.get_help() {
                format!("{compl}\t{help}")
            } else {
                compl.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
