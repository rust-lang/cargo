use std::path::PathBuf;

use cargo::util::command_prelude::*;
use cargo_test_support::cargo_test;

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
