use std::path::PathBuf;

/// A list of shell command arguments.
pub type Args = Vec<Arg>;

/// Represents a shell command argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// A raw byte sequence argument with no further interpretation.
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
    fn test_arg_display() {
        let arg = Arg::from("display_test");
        assert_eq!(arg.to_string(), "display_test");
    }

    #[test]
    fn test_arg_equality() {
        let arg1 = Arg::from("same");
        let arg2 = Arg::from("same");
        assert_eq!(arg1, arg2);
    }

    #[test]
    fn test_arg_to_pathbuf() {
        let arg = Arg::from("/path/to/file");
        let pathbuf = PathBuf::from(&arg);
        assert_eq!(pathbuf, PathBuf::from("/path/to/file"));
    }

}
