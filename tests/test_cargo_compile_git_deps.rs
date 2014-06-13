use support::{ProjectBuilder,ResultTest,project,execs,main_file};
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::{CargoResult,process};

fn setup() {
}

fn git_repo(name: &str, callback: |ProjectBuilder| -> ProjectBuilder) -> CargoResult<ProjectBuilder> {
    let mut git_project = project(name);
    git_project = callback(git_project);
    git_project.build();

    log!(5, "git init");
    try!(git_project.process("git").args(["init"]).exec_with_output());
    log!(5, "building git project");
    log!(5, "git add .");
    try!(git_project.process("git").args(["add", "."]).exec_with_output());
    log!(5, "git commit");
    try!(git_project.process("git").args(["commit", "-m", "Initial commit"]).exec_with_output());
    Ok(git_project)
}

test!(cargo_compile_simple_git_dep {
    let project = project("foo");
    let git_project = git_repo("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [[lib]]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).assert();

    let project = project
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            version = "0.5.0"
            git = "file://{}"

            [[bin]]

            name = "foo"
        "#, git_project.root().display()))
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, ["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("cargo-compile"),
        execs()
        .with_stdout(format!("Updating git repository `file:{}`\nCompiling dep1 v0.5.0 (file:{})\nCompiling foo v0.5.0 (file:{})\n",
                             git_root.display(), git_root.display(), root.display()))
        .with_stderr(""));

    assert_that(&project.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(project.root().join("target")),
      execs().with_stdout("hello world\n"));
})
