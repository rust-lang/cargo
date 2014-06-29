#![crate_id="cargo-init"]
#![feature(phase)]
#![allow(unused_must_use)]

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
    print!("{} [Default: {}]: ", query, default);
    let result = io::stdin().read_line().unwrap().to_str();
    if result == "\n".to_str()  {default.clone()}
    else {;result.as_slice().slice(0,result.len()-1).to_str().clone()}
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
        [ref nm, ref lic, ..] if *nm != "--override".to_str() =>make_cargo(nm.as_slice(), lic.as_slice()),
        _ => make_cargo_interactive()
    }
}

/// Prompts the user to name and license their project.
fn make_cargo_interactive(){
    make_cargo(ask_default("Project name ".to_str(),"Untitled project".to_str()).as_slice(),
               ask_default("License ".to_str(), "BSD3".to_str()).as_slice());
}

/// Write the .toml file and set up the .src directory with a dummy file
fn make_cargo(nm: &str, lic: &str){
    let mut tomlFile = io::fs::File::create(&Path::new("Cargo.toml"));
    
    io::fs::mkdir(&Path::new("src"), io::UserRWX);
    let mut srcFile = io::fs::File::create(&Path::new(format!("./src/{}.rs", nm)));
    srcFile.write("".as_bytes());

    tomlFile.write        ("[package]\n".as_slice().clone().as_bytes());
    tomlFile.write(format!("name = \"{}\"\n", nm).as_slice().clone().as_bytes());
    tomlFile.write        ("version = \"0.1.0\"\n".as_slice().clone().as_bytes());
    tomlFile.write(format!("license = \"{}\"\n\n", lic).as_slice().as_bytes());

    let userNameOpt = os::getenv("USER");
    tomlFile.write(format!("authors = [ \"{}\" ]\n", match userNameOpt{
        Some(a) => a,
        None => {println!("$USER environment variable not set. Assigning placeholder"); "YOUR_USER_NAME_HERE".to_str()}
    }).as_bytes()).unwrap();
    
    tomlFile.write        ("[[bin]]\n\n".as_slice().as_bytes());
    tomlFile.write(format!("name = \"{}\"\n", nm).as_slice().as_bytes());                                       
}

fn main() { execute();}
