#[macro_use]
extern crate duct;

#[test]
fn fixtures() {
    // ignore if this fails
    let _ = cmd!("cargo", "install", "clippy").run();
    let root_dir = std::env::current_dir().unwrap();
    println!("root: {:?}", root_dir);
    for fixture in std::fs::read_dir(root_dir.join("tests/crates")).unwrap() {
        let fixture = fixture.unwrap();
        let fixture_path = fixture.path();
        // FIXME: don't expect the crate to be in the `ui` subdir
        let dir = fixture_path.join("ui");
        for entry in std::fs::read_dir(root_dir.join("tests/tests").join(fixture_path.file_name().unwrap())).unwrap() {
            let test = entry.unwrap().path();
            println!("{:?}", test);
            assert!(cmd!("git", "checkout", ".").dir(&dir).run().unwrap().status.success());
            assert!(cmd!("cargo", "clean").dir(&dir).run().unwrap().status.success());
            let manifest = format!("{:?}", root_dir.join("Cargo.toml"));
            cmd!("cargo",
                "run",
                format!("--manifest-path={}", &manifest[1..manifest.len() - 1]),
                "--",
                "--clippy"
                )
                .dir(&dir)
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
        }
    }
}
