#![crate_id="cargo-init"]
#![feature(phase)]

use std::io;
use std::os;

/// Checks the existance of a file with a given extension.
fn already_exists(items: Vec<Path>, quer: &str) -> bool {
    items.iter().any(|nms: &Path| -> (bool) {
        let extStr = nms.extension_str();
        match extStr{
            Some(a) => a == quer,
            None => false
        }
    })

}

/// Prompts the user for command-line input, with a default response.
fn ask_default(query: String, default: String) -> String{
    let result = io::stdin().read_line().unwrap().to_str();
    if result == "\n".to_str()  {        
        default.clone()
    }
    else {
        result.as_slice().slice(0,result.len()-1).to_str().clone()        
    }
}

/// Initalize a Cargo build.
fn execute(){
    let osArgs = os::args();
    let mut args = osArgs.slice(1, osArgs.len()).iter().map(|x| x.as_slice()); // Drop program name

    let cwd_contents = io::fs::readdir(&os::getcwd()).unwrap();

    if already_exists(cwd_contents, "toml") && ! args.any(|x| x == "--override"){
        println!(".toml file already exists in current directory. Use --override to bypass.");
        return;
    }
    
    //Either explicitly state a name and license, or both will be gathered interactively.
    match osArgs.as_slice().slice(1,osArgs.len()){
        [ref nm, ref bl, ref auth, ..] if *nm != "--override".to_str() => make_cargo(nm.as_slice(), bl == &"lib".to_str(), auth.as_slice()),
        _ => make_cargo_interactive()
    }
}

/// Prompts the user to name and license their project.
fn make_cargo_interactive(){
    make_cargo(ask_default("Project name ".to_str(),"Untitled project".to_str()).as_slice(),
               ask_default("lib/bin? ".to_str(), "lib".to_str()).to_str().as_slice() == "lib".to_str().as_slice(),
               ask_default("Your name: ".to_str(), "anonymous".to_str()).as_slice());
}

/// Write the .toml file and set up the .src directory with a dummy file
fn make_cargo(nm: &str, lib: bool, auth: &str){
    let cwd_contents = io::fs::readdir(&os::getcwd()).unwrap();
    let mut tomlFile = io::fs::File::create(&Path::new("Cargo.toml"));
    let mut gitignr  = io::fs::File::create(&Path::new(".gitignore")); // Ignores "target" by default"

    let IOio::fs::mkdir(&Path::new("src"), io::UserRWX);
    let mut srcFile = io::fs::File::create(&Path::new(format!("./src/{}.rs", nm)));
    
    if !cwd_contents.iter().any(|x| x.filename_str().unwrap() == ".gitignore") {
        srcFile.write("".as_bytes());
        gitignr.write("target".as_bytes());
    }

    tomlFile.write        ("[package]\n".as_slice().clone().as_bytes());
    tomlFile.write(format!("name = \"{}\"\n", nm).as_slice().clone().as_bytes());
    tomlFile.write        ("version = \"0.1.0\"\n".as_slice().clone().as_bytes());

    tomlFile.write(format!("authors = [ \"{}\" ]\n\n", auth).as_bytes());
    
    if lib{
        tomlFile.write        ("[[lib]]\n".as_slice().as_bytes());
    } else
    {   tomlFile.write("[[bin]]\n".as_slice().as_bytes()); }

    tomlFile.write(format!("name = \"{}\"\n", nm).as_slice().as_bytes());
    
}

fn main() { execute();}
