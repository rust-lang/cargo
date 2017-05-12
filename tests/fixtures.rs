use std::process::{Command, Stdio};
use std::fs::File;
use std::io::{Read, Write};

#[test]
fn fixtures() {
    // ignore if this fails
    let _ = Command::new("cargo").arg("install").arg("clippy").status().unwrap();
    let root_dir = std::env::current_dir().unwrap();
    println!("root: {:?}", root_dir);
    for fixture in std::fs::read_dir(root_dir.join("tests/crates")).unwrap() {
        let fixture = fixture.unwrap();
        let fixture_path = fixture.path();
        // FIXME: don't expect the crate to be in the `ui` subdir
        std::env::set_current_dir(fixture_path.join("ui")).unwrap();
        for entry in std::fs::read_dir(root_dir.join("tests/tests").join(fixture_path.file_name().unwrap())).unwrap() {
            let test = entry.unwrap().path();
            println!("{:?}", test);
            assert!(Command::new("git").arg("checkout").arg(".").status().unwrap().success());
            assert!(Command::new("cargo").arg("clean").status().unwrap().success());
            let mut input = Vec::new();
            File::open(test.join("input.txt")).unwrap().read_to_end(&mut input).unwrap();
            let manifest = format!("{:?}", root_dir.join("Cargo.toml"));
            let mut cmd = Command::new("cargo")
                .arg("run")
                .arg(format!("--manifest-path={}", &manifest[1..manifest.len() - 1]))
                .arg("--")
                .arg("--clippy")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            cmd.stdin.as_mut().unwrap().write(&input).unwrap();
            let output = cmd.wait_with_output().unwrap();
            File::create("output.txt").unwrap().write_all(&output.stdout).unwrap();
            if std::env::var("APPLY_RUSTFIX").is_ok() {
                std::fs::copy("output.txt", test.join("output.txt")).unwrap();
                let diff = Command::new("git").arg("diff").output().unwrap();
                File::create(test.join("diff.diff")).unwrap().write_all(&diff.stdout).unwrap();
            } else {
                let diff = Command::new("git").arg("diff").output().unwrap();
                File::create("diff.diff").unwrap().write_all(&diff.stdout).unwrap();
                if !Command::new("diff").arg("-q").arg("diff.diff").arg(test.join("diff.diff")).status().unwrap().success() {
                    panic!("Unexpected changes applied by rustfix");
                }
                if !Command::new("diff").arg("-q").arg("output.txt").arg(test.join("output.txt")).status().unwrap().success() {
                    panic!("Unexpected output by rustfix");
                }
            }
        }
        std::env::set_current_dir(&root_dir).unwrap();
    }
}
