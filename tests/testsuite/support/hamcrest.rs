use std::fmt;
use std::marker;
use std::path::Path;

pub type MatchResult = Result<(), String>;

pub trait Matcher<T>: fmt::Debug {
    fn matches(&self, actual: T) -> Result<(), String>;
}

pub fn assert_that<T, U: Matcher<T>>(actual: T, matcher: U) {
    if let Err(e) = matcher.matches(actual) {
        panic!("\nExpected: {:?}\n    but: {}", matcher, e)
    }
}

pub fn existing_file() -> ExistingFile {
    ExistingFile
}

#[derive(Debug)]
pub struct ExistingFile;

impl<P> Matcher<P> for ExistingFile
where
    P: AsRef<Path>,
{
    fn matches(&self, actual: P) -> Result<(), String> {
        if actual.as_ref().is_file() {
            Ok(())
        } else {
            Err(format!("{} was not a file", actual.as_ref().display()))
        }
    }
}

pub fn existing_dir() -> ExistingDir {
    ExistingDir
}

#[derive(Debug)]
pub struct ExistingDir;

impl<P> Matcher<P> for ExistingDir
where
    P: AsRef<Path>,
{
    fn matches(&self, actual: P) -> Result<(), String> {
        if actual.as_ref().is_dir() {
            Ok(())
        } else {
            Err(format!("{} was not a dir", actual.as_ref().display()))
        }
    }
}

pub fn is_not<T, M: Matcher<T>>(matcher: M) -> IsNot<T, M> {
    IsNot {
        matcher,
        _marker: marker::PhantomData,
    }
}

#[derive(Debug)]
pub struct IsNot<T, M> {
    matcher: M,
    _marker: marker::PhantomData<T>,
}

impl<T, M: Matcher<T>> Matcher<T> for IsNot<T, M>
where
    T: fmt::Debug,
{
    fn matches(&self, actual: T) -> Result<(), String> {
        match self.matcher.matches(actual) {
            Ok(_) => Err("matched".to_string()),
            Err(_) => Ok(()),
        }
    }
}

pub fn contains<T>(item: Vec<T>) -> Contains<T> {
    Contains(item)
}

#[derive(Debug)]
pub struct Contains<T>(Vec<T>);

impl<'a, T> Matcher<&'a Vec<T>> for Contains<T>
where
    T: fmt::Debug + PartialEq,
{
    fn matches(&self, actual: &'a Vec<T>) -> Result<(), String> {
        for item in self.0.iter() {
            if !actual.contains(item) {
                return Err(format!("failed to find {:?}", item));
            }
        }
        Ok(())
    }
}
