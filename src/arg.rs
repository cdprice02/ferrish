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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_from_str() {
        let arg = Arg::from("hello");
        match arg {
            Arg::Literal(bytes) => assert_eq!(bytes, b"hello"),
        }
    }

    #[test]
    #[ignore]
    fn test_arg_from_bytes_slice() {
        let arg = Arg::from(b"world" as &[u8]);
        match arg {
            Arg::Literal(bytes) => assert_eq!(bytes, b"world"),
        }
    }

    #[test]
    #[ignore]
    fn test_arg_from_vec() {
        let vec = b"test".to_vec();
        let arg = Arg::from(vec);
        match arg {
            Arg::Literal(bytes) => assert_eq!(bytes, b"test"),
        }
    }

    #[test]
    fn test_arg_display() {
        let arg = Arg::from("display_test");
        assert_eq!(arg.to_string(), "display_test");
    }

    #[test]
    #[ignore]
    fn test_arg_display_empty() {
        let arg = Arg::from("");
        assert_eq!(arg.to_string(), "");
    }

    #[test]
    fn test_arg_equality() {
        let arg1 = Arg::from("same");
        let arg2 = Arg::from("same");
        assert_eq!(arg1, arg2);
    }

    #[test]
    #[ignore]
    fn test_arg_inequality() {
        let arg1 = Arg::from("first");
        let arg2 = Arg::from("second");
        assert_ne!(arg1, arg2);
    }

    #[test]
    fn test_arg_to_pathbuf() {
        let arg = Arg::from("/path/to/file");
        let pathbuf = PathBuf::from(&arg);
        assert_eq!(pathbuf, PathBuf::from("/path/to/file"));
    }

    #[test]
    #[ignore]
    fn test_arg_with_special_chars() {
        let arg = Arg::from("file-with_special.chars");
        assert_eq!(arg.to_string(), "file-with_special.chars");
    }

    #[test]
    #[ignore]
    fn test_arg_clone() {
        let arg1 = Arg::from("cloneable");
        let arg2 = arg1.clone();
        assert_eq!(arg1, arg2);
    }
}
