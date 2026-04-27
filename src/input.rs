use miette::NamedSource;

/// A raw input line as received from the user, with the trimmed view and
/// leading-whitespace offset pre-computed.
///
/// The parser operates on [`Input::trimmed_bytes`] but constructs all
/// diagnostic spans as absolute offsets into [`Input::raw_str`], so errors
/// are self-contained and render correctly without any adjustment at the
/// call site.
pub struct Input {
    raw: String,
    /// Byte offset of the first non-whitespace character in `raw`.
    leading_offset: usize,
    /// Byte length of the trimmed content (trailing whitespace excluded).
    trimmed_len: usize,
}

impl Input {
    /// Construct an [`Input`] from raw bytes as received from the user.
    pub fn new(bytes: &[u8]) -> Self {
        let raw = String::from_utf8_lossy(bytes).into_owned();
        let raw_bytes = raw.as_bytes();
        let leading_offset = raw_bytes.len() - raw_bytes.trim_ascii_start().len();
        let trimmed_len = raw_bytes.trim_ascii().len();
        Self { raw, leading_offset, trimmed_len }
    }

    /// The slice the parser operates on — leading and trailing whitespace removed.
    pub fn trimmed_bytes(&self) -> &[u8] {
        &self.raw.as_bytes()[self.leading_offset..self.leading_offset + self.trimmed_len]
    }

    /// Byte offset of the trimmed content within the raw string.
    pub fn leading_offset(&self) -> usize {
        self.leading_offset
    }

    /// Returns `true` if the input contains only whitespace.
    pub fn is_effectively_empty(&self) -> bool {
        self.trimmed_len == 0
    }

    /// The original input string as received, including all whitespace.
    pub fn raw_str(&self) -> &str {
        &self.raw
    }

    /// A named miette source wrapping the raw input line, suitable for
    /// embedding directly in diagnostic variants via `#[source_code]`.
    pub fn named_source(&self) -> NamedSource<String> {
        NamedSource::new("<input>", self.raw.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trimmed_bytes_strips_whitespace() {
        let input = Input::new(b"  echo hello  \n");
        assert_eq!(input.trimmed_bytes(), b"echo hello");
    }

    #[test]
    fn leading_offset_counts_leading_bytes() {
        let input = Input::new(b"  echo hello");
        assert_eq!(input.leading_offset(), 2);
    }

    #[test]
    fn leading_offset_zero_for_no_leading_whitespace() {
        let input = Input::new(b"echo hello");
        assert_eq!(input.leading_offset(), 0);
    }

    #[test]
    fn is_effectively_empty_for_whitespace_only() {
        assert!(Input::new(b"   \t\n").is_effectively_empty());
        assert!(Input::new(b"").is_effectively_empty());
    }

    #[test]
    fn is_effectively_empty_false_for_content() {
        assert!(!Input::new(b"  echo  ").is_effectively_empty());
    }

    #[test]
    fn raw_str_preserves_original() {
        let input = Input::new(b"  echo hello\n");
        assert_eq!(input.raw_str(), "  echo hello\n");
    }
}
