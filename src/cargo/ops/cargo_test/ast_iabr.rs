// ast_iabr.rs
use crate::util::{CliResult, GlobalContext};
use crate::core::Workspace;
use std::io::{self, Write};
use std::fs;
use std::path::{Path, PathBuf};
use syn::File;
use std::collections::HashMap;
use prettyplease;


pub fn create_trees(ws: &Workspace<'_>) -> syn::Result<HashMap<PathBuf, File>>
{
    // Create the empty to hash map to store in later
    let mut trees = HashMap::new();

    for package in ws.members()
    {
        // This one is interesting. We want to test all the files in test_cargo/ 
        // but package.manifest_path gives the test_cargo/cargo.toml
        // We need to move up from that, thus the package root
        let toml_path = package.manifest_path();
        let package_root = toml_path.parent().unwrap();

        // Find the main file (Prevents the next step from erroring out if no file was found)
        let main_file = package_root.join("src/main.rs");
        // Check if it exists
        if !main_file.exists()
        {
            // Remember, continue skips the current iteration of the loop, or this entire package
            continue;
        }

        // Now lets read in the file
        let source = fs::read_to_string(&main_file)
            .expect("Failed to open file");

        let ast: File = syn::parse_file(&source)?;
        eprintln!("Tree created for {:?}", main_file);

        // Print out the tree
        let reformatted_ast = prettyplease::unparse(&ast);
        eprintln!("{}", reformatted_ast);
        trees.insert(main_file, ast);

    }

    Ok(trees)

}