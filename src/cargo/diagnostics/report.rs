use std::borrow::Cow;
use std::ops::Range;
use std::path::Path;

use pathdiff::diff_paths;

use crate::GlobalContext;

/// Gets the relative path to a manifest from the current working directory, or
/// the absolute path of the manifest if a relative path cannot be constructed
pub fn rel_cwd_manifest_path(path: &Path, gctx: &GlobalContext) -> String {
    diff_paths(path, gctx.cwd())
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

pub fn get_key_value<'doc, 'i>(
    document: &'doc toml::Spanned<toml::de::DeTable<'static>>,
    path: &[impl AsIndex],
) -> Option<(
    &'doc toml::Spanned<Cow<'doc, str>>,
    &'doc toml::Spanned<toml::de::DeValue<'static>>,
)> {
    let table = document.get_ref();
    let mut iter = path.into_iter();
    let index0 = iter.next()?.as_index();
    let key0 = index0.as_key()?;
    let (mut current_key, mut current_item) = table.get_key_value(key0)?;

    while let Some(index) = iter.next() {
        match index.as_index() {
            TomlIndex::Key(key) => {
                if let Some(table) = current_item.get_ref().as_table() {
                    (current_key, current_item) = table.get_key_value(key)?;
                } else if let Some(array) = current_item.get_ref().as_array() {
                    current_item = array.iter().find(|item| match item.get_ref() {
                        toml::de::DeValue::String(s) => s == key,
                        _ => false,
                    })?;
                } else {
                    return None;
                }
            }
            TomlIndex::Offset(offset) => {
                let array = current_item.get_ref().as_array()?;
                current_item = array.get(offset)?;
            }
        }
    }
    Some((current_key, current_item))
}

pub fn get_key_value_span<'i>(
    document: &toml::Spanned<toml::de::DeTable<'static>>,
    path: &[impl AsIndex],
) -> Option<TomlSpan> {
    get_key_value(document, path).map(|(k, v)| TomlSpan {
        key: k.span(),
        value: v.span(),
    })
}

#[derive(Clone)]
pub struct TomlSpan {
    pub key: Range<usize>,
    pub value: Range<usize>,
}

#[derive(Copy, Clone)]
pub enum TomlIndex<'i> {
    Key(&'i str),
    Offset(usize),
}

impl<'i> TomlIndex<'i> {
    fn as_key(&self) -> Option<&'i str> {
        match self {
            TomlIndex::Key(key) => Some(key),
            TomlIndex::Offset(_) => None,
        }
    }
}

pub trait AsIndex {
    fn as_index<'i>(&'i self) -> TomlIndex<'i>;
}

impl AsIndex for TomlIndex<'_> {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        match self {
            TomlIndex::Key(key) => TomlIndex::Key(key),
            TomlIndex::Offset(offset) => TomlIndex::Offset(*offset),
        }
    }
}

impl AsIndex for &str {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        TomlIndex::Key(self)
    }
}

impl AsIndex for String {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        TomlIndex::Key(self.as_str())
    }
}

impl AsIndex for usize {
    fn as_index<'i>(&'i self) -> TomlIndex<'i> {
        TomlIndex::Offset(*self)
    }
}
