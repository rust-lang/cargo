use std::fmt;

pub type MatchResult = Result<(), String>;

pub trait Matcher<T>: fmt::Debug {
    fn matches(&self, actual: T) -> Result<(), String>;
}
