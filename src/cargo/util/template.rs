use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Read};

use util::{CargoResult, human, ChainError};
use url::Url;

use handlebars::{Helper, Handlebars, RenderContext, RenderError, html_escape};
use tempdir::TempDir;
use toml;

/// toml_escape_helper quotes strings in templates when they are wrapped in
/// {{#toml-escape <template-variable}}
/// So if 'name' is "foo \"bar\"" then:
/// {{name}} renders as  'foo "bar"'
/// {{#toml-escape name}} renders as '"foo \"bar\""'
pub fn toml_escape_helper(h: &Helper,
                          _: &Handlebars,
                          rc: &mut RenderContext) -> Result<(), RenderError> {
    if let Some(param) = h.param(0) {
        let txt = param.value().as_string().unwrap_or("").to_owned();
        let rendered = format!("{}", toml::Value::String(txt));
        try!(rc.writer.write_all(rendered.into_bytes().as_ref()));
    }
    Ok(())
}

/// html_escape_helper escapes strings in templates using html escaping rules.
pub fn html_escape_helper(h: &Helper,
                          _: &Handlebars,
                          rc: &mut RenderContext) -> Result<(), RenderError> {
    if let Some(param) = h.param(0) {
        let rendered = html_escape(param.value().as_string().unwrap_or(""));
        try!(rc.writer.write_all(rendered.into_bytes().as_ref()));
    }
    Ok(())
}

/// Trait to hold information required for rendering templated files.
pub trait TemplateFile {
    /// Path of the template output for the file being written.
    fn path(&self) -> &Path;

    /// Return the template string.
    fn template(&self) -> CargoResult<String>;
}

/// TemplateFile based on an input file.
pub struct InputFileTemplateFile {
    input_path: PathBuf,
    output_path: PathBuf,
}

impl TemplateFile for InputFileTemplateFile {
    fn path(&self) -> &Path {
        &self.output_path
    }

    fn template(&self) -> CargoResult<String> {
        let mut template_str = String::new();
        let mut entry_file = try!(File::open(&self.input_path).chain_error(|| {
            human(format!("Failed to open file for templating: {}", self.input_path.display()))
        }));
        try!(entry_file.read_to_string(&mut template_str).chain_error(|| {
            human(format!("Failed to read file for templating: {}", self.input_path.display()))
        }));
        Ok(template_str)
    }
}

impl InputFileTemplateFile {
    pub fn new(input_path: PathBuf, output_path: PathBuf) -> InputFileTemplateFile {
        InputFileTemplateFile {
            input_path: input_path,
            output_path: output_path
        }
    }
}

/// An in memory template file for --bin or --lib.
pub struct InMemoryTemplateFile {
    template_str: String,
    output_path: PathBuf,
}

impl TemplateFile for InMemoryTemplateFile {
    fn path(&self) -> &Path {
        &self.output_path
    }

    fn template(&self) -> CargoResult<String> {
        Ok(self.template_str.clone())
    }
}

impl InMemoryTemplateFile {
    pub fn new(output_path: PathBuf, template_str: String) -> InMemoryTemplateFile {
        InMemoryTemplateFile {
            template_str: template_str,
            output_path: output_path
        }
    }
}

pub enum TemplateDirectory{
    Temp(TempDir),
    Normal(PathBuf),
}

impl TemplateDirectory {
    pub fn path(&self) -> &Path {
        match *self {
            TemplateDirectory::Temp(ref tempdir) => tempdir.path(),
            TemplateDirectory::Normal(ref path) => path.as_path()
        }
    }
}

/// A listing of all the files that are part of the template.
pub struct TemplateSet {
    pub template_dir: Option<TemplateDirectory>,
    pub template_files: Vec<Box<TemplateFile>>
}

// The type of template we will use.
#[derive(Debug, Eq, PartialEq)]
pub enum TemplateType  {
    GitRepo(String),
    LocalDir(String),
    Builtin
}

/// Given a repository string and subdir, determine if this is a git repository, local file, or a
/// built in template. Git only supports a few schemas, so anything that is not supported is
/// treated as a local path. The supported schemes are:
/// "git", "file", "http", "https", and "ssh"
/// Also supported is an scp style syntax: git@domain.com:user/path
pub fn get_template_type<'a>(repo: Option<&'a str>,
                             subdir: Option<&'a str>) -> CargoResult<TemplateType> {
    match (repo, subdir) {
        (Some(repo_str), _) => {
            if let Ok(repo_url) = Url::parse(repo_str) {
                let supported_schemes = ["git", "file", "http", "https", "ssh"];
                if supported_schemes.contains(&repo_url.scheme()) {
                    Ok(TemplateType::GitRepo(repo_url.into_string()))
                } else {
                    Ok(TemplateType::LocalDir(String::from(repo_str)))
                }
            } else {
                Ok(TemplateType::LocalDir(String::from(repo_str)))
            }
        },
        (None, Some(_)) => Err(human("A template was given, but no template repository")),
        (None, None) => Ok(TemplateType::Builtin)
    }
}


#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use handlebars::Handlebars;
    use super::*;

    #[test]
    fn test_toml_escape_helper() {
        let mut handlebars = Handlebars::new();
        handlebars.register_helper("toml-escape", Box::new(toml_escape_helper));
        let mut data = BTreeMap::new();
        data.insert("name".to_owned(), "\"Iron\" Mike Tyson".to_owned());
        let template_string = r#"Hello, {{#toml-escape name}}{{/toml-escape}}"#;
        let result = handlebars.template_render(template_string, &data).unwrap();
        assert_eq!(result, "Hello, \"\\\"Iron\\\" Mike Tyson\"");
    }

    macro_rules! test_get_template_proto {
        ( $funcname:ident, $url:expr ) => {
            #[test]
            fn $funcname() {
                assert_eq!(get_template_type(Some($url), Some("foo")).unwrap(),
                TemplateType::GitRepo($url.to_owned()));
                assert_eq!(get_template_type(Some($url), Some("")).unwrap(),
                TemplateType::GitRepo($url.to_owned()));
                assert_eq!(get_template_type(Some($url), None).unwrap(),
                TemplateType::GitRepo($url.to_owned()));
            }
        }
    }

    test_get_template_proto!(test_get_template_http, "http://foo.com/user/repo");
    test_get_template_proto!(test_get_template_https, "https://foo.com/user/repo");
    test_get_template_proto!(test_get_template_git, "git://foo.com/user/repo");
    test_get_template_proto!(test_get_template_file, "file://foo.com/user/repo");
    test_get_template_proto!(test_get_template_ssh, "ssh://user@foo.com/repo");
    // SSH scp style repository access is not yet supported.
    //test_get_template_proto!(test_get_template_ssh_scp_style, "git@foo.com:user/repo");

    #[test]
    fn test_get_template_type_git_repo_bad_proto_is_localdir() {
        assert_eq!(get_template_type(Some("ftps://foo.com/user/repo"), None).unwrap(),
                   TemplateType::LocalDir("ftps://foo.com/user/repo".to_owned()));
    }

    #[test]
    fn test_get_template_type_local_dir_abs() {
        assert_eq!(get_template_type(Some("/foo/user/repo"), Some("foo")).unwrap(),
                   TemplateType::LocalDir("/foo/user/repo".to_owned()));
        assert_eq!(get_template_type(Some("/foo/user/repo"), Some("")).unwrap(),
                   TemplateType::LocalDir("/foo/user/repo".to_owned()));
        assert_eq!(get_template_type(Some("/foo/user/repo"), None).unwrap(),
                   TemplateType::LocalDir("/foo/user/repo".to_owned()));
    }

    // Windows paths can be parsed as URLs so make sure they are parsed as local directories.
    #[test]
    fn test_get_template_type_windows_path_is_localdir() {
        assert_eq!(get_template_type(Some(r#"C:\foo\user\repo"#), None).unwrap(),
                   TemplateType::LocalDir(r#"C:\foo\user\repo"#.to_owned()));
        assert_eq!(get_template_type(Some(r#"C:/foo/user/repo"#), None).unwrap(),
                   TemplateType::LocalDir(r#"C:/foo/user/repo"#.to_owned()));
    }

    #[test]
    fn test_get_template_type_local_dir_rel() {
        assert_eq!(get_template_type(Some("foo/user/repo"), Some("foo")).unwrap(),
                   TemplateType::LocalDir("foo/user/repo".to_owned()));
        assert_eq!(get_template_type(Some("foo/user/repo"), Some("")).unwrap(),
                   TemplateType::LocalDir("foo/user/repo".to_owned()));
        assert_eq!(get_template_type(Some("foo/user/repo"), None).unwrap(),
                   TemplateType::LocalDir("foo/user/repo".to_owned()));
    }

    #[test]
    fn test_get_template_type_builtin() {
        assert_eq!(get_template_type(None, None).unwrap(), TemplateType::Builtin);
    }
}
