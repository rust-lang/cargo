// mutation_iabr.rs
use crate::util::{CliResult, GlobalContext};
use crate::core::Workspace;
use std::io::{self, Write};
use std::fs;
use std::path::{Path, PathBuf};

// Import Syn parsing program
use super::ast_iabr;


/// Temporary proof of concept code to print something
pub fn run_mutations(_ws: &Workspace<'_>) -> CliResult 
{
    eprintln!("Mutation call success");
   // io::stdout().flush().unwrap(); // make sure it prints immediately
    Ok(())
}

pub fn list_files(ws: &Workspace<'_>) -> CliResult
{
    // Set the root of this function to the top level of the passed workspace (The folder you are running this command in)
    let root = ws.root();

    // Basic print messages
    eprintln!("\nWorkspace Root: {}", root.display());

    // Declare the vector to store files 
    // Vec works dynamically so you dont need to size it
    let mut rust_files = Vec::new();

    // For every package in the directory  (TOML file) scan them and find all .rs files
    for package in ws.members()
    {
        let package_root = package.root();
        eprintln!("Scanning Package: {}", package.name());
        find_rust_files(package_root, &mut rust_files)?;
    }

    // Print the results
    eprintln!("Files to be tested");
    for file in rust_files
    {
        eprintln!("   {}", file.display());
    }

    // Create the ASTs
    eprintln!("\nCREATING ABSTRACT SYNTAX TREES");
    let _trees = ast_iabr::create_trees(ws);


    Ok(())

}

fn find_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> CliResult
{
    // For each thing in the passed directory
    for each in fs::read_dir(dir)?
    {
        // Get anything, if nothing error out
        let file = each?;

        // Get the path of the anything
        let path = file.path();

        // Check if the anything is a folder
        if path.is_dir() 
        {
            // Check if the folder is a target or git folder and skip it (Time saving)
            if path.ends_with("target") || path.ends_with(".git")
            {
            continue;
            }

            // Search the folder (Recursive call)
            find_rust_files(&path, files)?;
        }else
        {
            // Because .extension() can return a null extension from things like makefile, we use Some(ext) to filter for only extensions
            if let Some(ext) = path.extension()
            {
                // Now look for any rs file
                if ext == "rs"
                {
                    files.push(path);
                }
            }
        }
    }

    Ok(())
}