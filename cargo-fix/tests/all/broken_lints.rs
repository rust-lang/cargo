//! Ensure we give good error message when rustfix failes to apply changes
//!
//! TODO: Add rustc shim that outputs wrong suggestions instead of depending on
//! actual rustc bugs!

// use super::project;

// #[test]
// fn tell_user_about_broken_lints() {
//     let p = project()
//         .file(
//             "src/lib.rs",
//             r#"
//                 pub fn foo() {
//                     let mut i = 42;
//                 }
//             "#,
//         )
//         .build();

//     p.expect_cmd("cargo-fix fix")
//         .env("__CARGO_FIX_YOLO", "true")
//         .stderr_contains(r"warning: error applying suggestions to `src/lib.rs`")
//         .stderr_contains("The full error message was:")
//         .stderr_contains("> Could not replace range 56...60 in file -- maybe parts of it were already replaced?")
//         .stderr_contains("\
//             This likely indicates a bug in either rustc or rustfix itself,\n\
//             and we would appreciate a bug report! You're likely to see \n\
//             a number of compiler warnings after this message which rustfix\n\
//             attempted to fix but failed. If you could open an issue at\n\
//             https://github.com/rust-lang-nursery/rustfix/issues\n\
//             quoting the full output of this command we'd be very appreciative!\n\n\
//         ")
//         .status(0)
//         .run();
// }
