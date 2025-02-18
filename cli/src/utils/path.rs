use std::{path::PathBuf, str::FromStr};

use ir::utils::diagnostic::Diagnostic;

#[derive(Debug, Clone)]
pub struct Path {
    segments: Vec<String>,
}

impl Path {
    pub const fn empty() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub const fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().map(|s| s.as_str())
    }

    pub fn pop(&mut self) -> Option<String> {
        self.segments.pop()
    }

    pub fn join<'a>(&self, other: &Path) -> Self {
        let mut segments = self.segments.clone();
        segments.extend(other.segments.iter().map(|s| s.clone()));
        Self { segments }
    }
}

impl FromStr for Path {
    type Err = Diagnostic;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(
            PathBuf::from(s)
                .iter()
                .map(|s| s.to_str().unwrap().to_owned())
                .collect(),
        ))
    }
}

impl ToString for Path {
    fn to_string(&self) -> String {
        self.segments.join("/")
    }
}

impl From<&str> for Path {
    fn from(value: &str) -> Self {
        Self::from_str(value).unwrap()
    }
}

pub struct PathIter<'a, I>
where
    I: Iterator<Item = &'a str>,
{
    iter: I,
}

impl<'a, I> Iterator for PathIter<'a, I>
where
    I: Iterator<Item = &'a str>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a, I> PathIter<'a, I>
where
    I: Iterator<Item = &'a str>,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }

    pub fn next_segment(&mut self) -> Result<&'a str, Diagnostic> {
        self.next()
            .ok_or(Diagnostic::error("path terminates early.".to_owned()))
    }
}
