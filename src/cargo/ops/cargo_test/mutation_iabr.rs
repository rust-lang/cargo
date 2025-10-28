// mutation_iabr.rs
use crate::util::{CliResult, GlobalContext};
use crate::core::Workspace;
use std::io::{self, Write};


/// Temporary proof of concept code to print something
pub fn run_mutations(_ws: &Workspace<'_>) -> CliResult 
{
    println!("Mutation call success");
   // io::stdout().flush().unwrap(); // make sure it prints immediately
    Ok(())
}