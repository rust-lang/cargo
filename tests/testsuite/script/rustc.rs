use std::collections::BTreeMap;

use snapbox::assert_data_eq;

use crate::prelude::*;

#[test]
fn ensure_all_fixtures_have_tests() {
    let mut code_gen_divider = "//".to_owned();
    code_gen_divider.push_str(" START CODE GENERATION");

    let self_path = snapbox::utils::current_rs!();
    let self_source = std::fs::read_to_string(&self_path).unwrap();

    let (header, _) = self_source
        .split_once(&code_gen_divider)
        .expect("code-gen divider is present");
    let header = header.trim();

    let fixture_root = snapbox::utils::current_dir!().join("rustc_fixtures");
    let mut fixtures = BTreeMap::new();
    for entry in std::fs::read_dir(fixture_root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let fn_name = file_to_fn(&path);
        fixtures
            .entry(fn_name.clone())
            .or_insert_with(|| Fixture::new(fn_name))
            .add_path(path);
    }

    let fixtures = fixtures
        .into_values()
        .filter(Fixture::is_valid)
        .map(|f| f.to_string())
        .collect::<String>();
    let actual = format!(
        "{header}

{code_gen_divider}
{fixtures}"
    );
    assert_data_eq!(actual, snapbox::Data::read_from(&self_path, None).raw());
}

fn file_to_fn(path: &std::path::Path) -> String {
    let name = path.file_stem().unwrap().to_str().unwrap();
    name.replace("-", "_")
}

fn sanitize_path(path: &std::path::Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
        .as_os_str()
        .to_string_lossy()
        .replace("\\", "/")
}

struct Fixture {
    fn_name: String,
    input: std::path::PathBuf,
    output: Option<std::path::PathBuf>,
}

impl Fixture {
    fn new(fn_name: String) -> Self {
        Self {
            fn_name,
            input: Default::default(),
            output: Default::default(),
        }
    }

    fn is_valid(&self) -> bool {
        !self.input.as_os_str().is_empty()
    }

    fn add_path(&mut self, path: std::path::PathBuf) {
        if path.extension().map(|ext| ext.to_str().unwrap()) == Some("rs") {
            assert!(
                self.input.as_os_str().is_empty(),
                "similarly named fixtures:/n{}/n{}",
                self.input.display(),
                path.display()
            );
            self.input = path;
        } else {
            assert!(
                self.output.is_none(),
                "conflicting assertions:/n{}/n{}",
                self.output.as_ref().unwrap().display(),
                path.display()
            );
            self.output = Some(path);
        }
    }
}

impl std::fmt::Display for Fixture {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fn_name = &self.fn_name;
        let fixture_path = sanitize_path(&self.input);
        match self
            .output
            .as_ref()
            .map(|path| path.extension().unwrap().to_str().unwrap())
        {
            Some("stderr") => {
                let assertion_path = sanitize_path(self.output.as_ref().unwrap());
                write!(
                    fmt,
                    r#"
#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn {fn_name}() {{
    let fixture_path = {fixture_path:?};
    let assertion_path = {assertion_path:?};
    assert_failure(fixture_path, assertion_path);
}}
"#
                )
            }
            Some("stdout") | None => {
                let mut backup_path = self.input.clone();
                backup_path.set_extension("stdout");
                let assertion_path = sanitize_path(self.output.as_ref().unwrap_or(&backup_path));
                write!(
                    fmt,
                    r#"
#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn {fn_name}() {{
    let fixture_path = {fixture_path:?};
    let assertion_path = {assertion_path:?};
    assert_success(fixture_path, assertion_path);
}}
"#
                )
            }
            Some(_) => {
                panic!(
                    "unsupported assertiong: {}",
                    self.output.as_ref().unwrap().display()
                )
            }
        }
    }
}

#[track_caller]
fn assert_success(fixture_path: &str, assertion_path: &str) {
    let p = cargo_test_support::project()
        .file("script", &std::fs::read_to_string(fixture_path).unwrap())
        .build();

    // `read-manifest` to validate frontmatter content without processing deps, compiling
    p.cargo("-Zscript read-manifest --manifest-path script")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(snapbox::Data::read_from(
            std::path::Path::new(assertion_path),
            Some(snapbox::data::DataFormat::Json),
        ))
        .run();
}

#[track_caller]
fn assert_failure(fixture_path: &str, assertion_path: &str) {
    let p = cargo_test_support::project()
        .file("script", &std::fs::read_to_string(fixture_path).unwrap())
        .build();

    // `read-manifest` to validate frontmatter content without processing deps, compiling
    p.cargo("-Zscript read-manifest --manifest-path script")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(snapbox::Data::read_from(
            std::path::Path::new(assertion_path),
            None,
        ))
        .run();
}

// START CODE GENERATION

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn dot_in_infostring_leading() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/dot-in-infostring-leading.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/dot-in-infostring-leading.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn dot_in_infostring_non_leading() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/dot-in-infostring-non-leading.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/dot-in-infostring-non-leading.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn escape() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/escape.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/escape.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn extra_after_end() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/extra-after-end.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/extra-after-end.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_after_tokens() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-after-tokens.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-after-tokens.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_contains_whitespace() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-contains-whitespace.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-contains-whitespace.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_crlf() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-crlf.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-crlf.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_inner_hyphens_1() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-inner-hyphens-1.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-inner-hyphens-1.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_inner_hyphens_2() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-inner-hyphens-2.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-inner-hyphens-2.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_non_lexible_tokens() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-non-lexible-tokens.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-non-lexible-tokens.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_whitespace_1() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-1.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-1.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_whitespace_2() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-2.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-2.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_whitespace_3() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-3.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-3.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn frontmatter_whitespace_4() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-4.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/frontmatter-whitespace-4.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn hyphen_in_infostring_leading() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/hyphen-in-infostring-leading.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/hyphen-in-infostring-leading.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn hyphen_in_infostring_non_leading() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/hyphen-in-infostring-non-leading.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/hyphen-in-infostring-non-leading.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn included_frontmatter() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/included-frontmatter.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/included-frontmatter.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn infostring_fail() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/infostring-fail.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/infostring-fail.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn mismatch_1() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/mismatch-1.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/mismatch-1.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn mismatch_2() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/mismatch-2.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/mismatch-2.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn multifrontmatter() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/multifrontmatter.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/multifrontmatter.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn multifrontmatter_2() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/multifrontmatter-2.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/multifrontmatter-2.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn proc_macro_observer() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/proc-macro-observer.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/proc-macro-observer.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn shebang() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/shebang.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/shebang.stdout";
    assert_success(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn unclosed_1() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/unclosed-1.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/unclosed-1.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn unclosed_2() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/unclosed-2.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/unclosed-2.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn unclosed_3() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/unclosed-3.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/unclosed-3.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn unclosed_4() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/unclosed-4.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/unclosed-4.stderr";
    assert_failure(fixture_path, assertion_path);
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[rustfmt::skip]  // code-generated
fn unclosed_5() {
    let fixture_path = "tests/testsuite/script/rustc_fixtures/unclosed-5.rs";
    let assertion_path = "tests/testsuite/script/rustc_fixtures/unclosed-5.stderr";
    assert_failure(fixture_path, assertion_path);
}
