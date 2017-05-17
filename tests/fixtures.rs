#[macro_use]
extern crate duct;

#[test]
fn fixtures() {
    // ignore if this fails
    let _ = cmd!("cargo", "install", "clippy").stderr_capture().run();
    let root_dir = std::env::current_dir().unwrap();

    println!("looking for fixtures in directory: {:?}", root_dir);

    for fixture in std::fs::read_dir(root_dir.join("tests/crates")).unwrap() {
        let fixture = fixture.unwrap();
        if !fixture.file_type().unwrap().is_dir() {
            continue;
        }
        let fixture_path = fixture.path();

        let file = std::fs::File::open(fixture_path.with_extension("sub-path"))
            .and_then(|mut file| {
                use std::io::Read;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                Ok(contents)
            });
        let dir = if let Ok(path) = file {
            fixture_path.join(path.trim())
        } else {
            fixture_path.clone()
        };
        println!("Running tests for {:?}", dir);
        let tests = std::fs::read_dir(root_dir.join("tests/tests").join(fixture_path.file_name().unwrap())).unwrap();

        for entry in tests {
            let test = entry.unwrap().path();
            let yolo = test.file_name().unwrap() == "yolo";

            println!("---");
            println!("Running test: {:?}", test);

            assert!(cmd!("git", "checkout", ".").dir(&dir).run().unwrap().status.success());
            assert!(cmd!("cargo", "clean").dir(&dir).run().unwrap().status.success());
            // we only want to rustfix the final project, not any dependencies
            assert!(cmd!("cargo", "build").dir(&dir).run().unwrap().status.success());

            let manifest = format!("{:?}", root_dir.join("Cargo.toml"));

            println!("Checking {:?} with clippy", test);

            let manifest = format!("--manifest-path={}", &manifest[1..manifest.len() - 1]);
            let cmd = if yolo {
                cmd!("cargo", "run", manifest, "--quiet", "--", "--clippy", "--yolo")
            } else {
                cmd!("cargo", "run", manifest, "--quiet", "--", "--clippy")
            };
            cmd.dir(&dir)
                .stdin(test.join("input.txt"))
                .stdout(dir.join("output.txt"))
                .run()
                .unwrap();

            if std::env::var("APPLY_RUSTFIX").is_ok() {
                std::fs::copy(dir.join("output.txt"), test.join("output.txt")).unwrap();

                cmd!("git", "diff").dir(&dir).stdout(test.join("diff.diff")).run().unwrap();
            } else {
                cmd!("git", "diff").dir(&dir).stdout(dir.join("diff.diff")).run().unwrap();

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

            println!("---");
            println!("Success!");
        }
    }
}
