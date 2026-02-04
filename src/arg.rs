use std::path::PathBuf;

use crate::{Command, parser::parse_command};

pub type Args = Vec<Arg>;

/// Represents a shell command argument
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    Literal(Vec<u8>),
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::Literal(bytes) => write!(f, "{}", String::from_utf8_lossy(bytes)),
        }
    }
}

impl From<&str> for Arg {
    fn from(val: &str) -> Self {
        Arg::from(val.as_bytes())
    }
}

impl From<&[u8]> for Arg {
    fn from(val: &[u8]) -> Self {
        Arg::from(val.to_vec())
    }
}

impl From<Vec<u8>> for Arg {
    fn from(val: Vec<u8>) -> Self {
        Arg::Literal(val)
    }
}

impl From<&Arg> for Command {
    fn from(val: &Arg) -> Self {
        match val {
            Arg::Literal(bytes) => parse_command(bytes),
        }
    }
}

impl From<&Arg> for PathBuf {
    fn from(val: &Arg) -> Self {
        match val {
            Arg::Literal(_) => PathBuf::from(val.to_string()),
        }
    }
}
