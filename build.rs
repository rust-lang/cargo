fn main() {
    use std::env;
    use std::error::Error;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    fn get_fixture_files() -> Result<Vec<PathBuf>, Box<Error>> {
        Ok(fs::read_dir("./tests/fixtures")?
            .into_iter()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .filter(|p| {
                let x = p.to_string_lossy();
                x.ends_with(".rs") && !x.ends_with(".fixed.rs")
            })
            .collect())
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("fixture_tests.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    for file in &get_fixture_files().unwrap() {
        write!(f,
            r#"
                #[test]
                #[allow(non_snake_case)]
                fn {name}() {{
                    let _ = env_logger::try_init();
                    test_rustfix_with_file("{filename}").unwrap();
                }}
            "#,
            name=file.file_stem().unwrap().to_str().unwrap(),
            filename=file.to_str().unwrap(),
        ).unwrap();
    }
}
