#[macro_use]
extern crate duct;

use std::io::{BufReader, BufRead};

#[test]
fn fixtures() {
    // ignore if this fails
    let _ = cmd!("cargo", "install", "clippy").stderr_capture().run();
    let root_dir = std::env::current_dir().unwrap();

    println!("looking for fixtures in directory: {:?}\n", root_dir);

    for fixture in std::fs::read_dir(root_dir.join("tests/crates")).unwrap() {
        let fixture = fixture.unwrap();
        if !fixture.file_type().unwrap().is_dir() {
            continue;
        }
        let fixture_path = fixture.path();

        let files: Vec<String> = std::fs::File::open(fixture_path.with_extension("sub-path"))
            .and_then(|file| BufReader::new(file).lines().collect()).unwrap_or_default();
        let dirs = if files.is_empty() {
            vec![fixture_path.clone()]
        } else {
            files.into_iter().map(|file| fixture_path.join(file.trim())).collect()
        };
        for dir in dirs {
            println!("====================================================================");
            println!("Running tests for {:?}\n", dir);
            let tests = std::fs::read_dir(root_dir.join("tests/tests").join(fixture_path.file_name().unwrap())).unwrap();

            for entry in tests {
                let test = entry.unwrap().path();
                let yolo = test.file_name().unwrap() == "yolo";

                println!("Running test: {}", test.file_name().unwrap().to_str().unwrap());

                assert!(cmd!("git", "checkout", ".").dir(&dir).run().unwrap().status.success());
                assert!(cmd!("cargo", "clean").dir(&dir).run().unwrap().status.success());
                // we only want to rustfix the final project, not any dependencies
                // we don't care if the final build succeeds, since we're also testing error suggestions
                let _ = cmd!("cargo", "build").dir(&dir).stderr_null().stdout_null().run();

                let manifest = format!("{:?}", root_dir.join("Cargo.toml"));

                println!("Running cargo clippy to obtain suggestions");

                let manifest = format!("--manifest-path={}", &manifest[1..manifest.len() - 1]);
                let cmd = if yolo {
                    cmd!("cargo", "run", manifest, "--bin", "rustfix", "--quiet", "--", "--clippy", "--yolo")
                } else {
                    cmd!("cargo", "run", manifest, "--bin", "rustfix", "--quiet", "--", "--clippy")
                };
                cmd.dir(&dir)
                    .stdin(test.join("input.txt"))
                    .stdout(dir.join("output.txt"))
                    .run()
                    .unwrap();

                if std::env::var("APPLY_RUSTFIX").is_ok() {
                    std::fs::copy(dir.join("output.txt"), test.join("output.txt")).unwrap();

                    cmd!("git", "diff", ".").dir(&dir).stdout(test.join("diff.diff")).run().unwrap();
                } else {
                    cmd!("git", "diff", ".").dir(&dir).stdout(dir.join("diff.diff")).run().unwrap();

                    let diff = cmd!("diff", dir.join("diff.diff"), test.join("diff.diff"))
                        .dir(&dir)
                        .stdout_capture()
                        .unchecked()
                        .run().unwrap();
                    if !diff.status.success() {
                        panic!("Unexpected changes by rustfix:\n{}", std::str::from_utf8(&diff.stdout).unwrap());
                    }

                    let output = cmd!("diff", dir.join("output.txt"), test.join("output.txt"))
                        .dir(&dir)
                        .stdout_capture()
                        .unchecked()
                        .run().unwrap();
                    if !output.status.success() {
                        panic!("Unexpected output by rustfix:\n{}", std::str::from_utf8(&output.stdout).unwrap());
                    }
                }

                println!("Success!\n");
            }
        }
    }
}
