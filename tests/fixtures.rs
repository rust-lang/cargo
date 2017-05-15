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
        let fixture_path = fixture.path();

        // FIXME: don't expect the crate to be in the `ui` subdir
        let dir = fixture_path.join("ui");
        let tests = std::fs::read_dir(root_dir.join("tests/tests").join(fixture_path.file_name().unwrap())).unwrap();

        for entry in tests {
            let test = entry.unwrap().path();
            let yolo = test.file_name().unwrap() == "yolo";

            println!("---");
            println!("Running test: {:?}", test);

            assert!(cmd!("git", "checkout", ".").dir(&dir).run().unwrap().status.success());
            assert!(cmd!("cargo", "clean").dir(&dir).run().unwrap().status.success());

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

                if !cmd!("diff", "-q", dir.join("diff.diff"), test.join("diff.diff")).dir(&dir).unchecked().run().unwrap().status.success() {
                    panic!("Unexpected changes applied by rustfix");
                }

                if !cmd!("diff", "-q", dir.join("output.txt"), test.join("output.txt")).dir(&dir).unchecked().run().unwrap().status.success() {
                    panic!("Unexpected output by rustfix");
                }
            }

            println!("---");
            println!("Success!");
        }
    }
}
