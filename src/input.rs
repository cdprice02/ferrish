use bytes::Bytes;
use miette::{MietteError, MietteSpanContents, SourceCode, SourceSpan, SpanContents};

/// A named, byte-accurate miette source backed by a [`Bytes`] buffer.
///
/// Implements [`SourceCode`] so it can be used directly as the `#[source_code]`
/// field in diagnostic variants. Cloning is cheap — the underlying [`Bytes`]
/// is reference-counted.
#[derive(Clone, Debug)]
pub struct InputSource {
    name: &'static str,
    bytes: Bytes,
}

impl InputSource {
    fn new(name: &'static str, bytes: Bytes) -> Self {
        Self { name, bytes }
    }

    /// A zero-content source for synthetic tokens that have no real input origin.
    pub(crate) fn synthetic() -> Self {
        Self {
            name: "<synthetic>",
            bytes: Bytes::new(),
        }
    }
}

impl SourceCode for InputSource {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn SpanContents<'a> + 'a>, MietteError> {
        let bytes: &'a [u8] = &self.bytes;
        let inner = bytes.read_span(span, context_lines_before, context_lines_after)?;
        Ok(Box::new(MietteSpanContents::new_named(
            self.name.to_string(),
            inner.data(),
            *inner.span(),
            inner.line(),
            inner.column(),
            inner.line_count(),
        )))
    }
}

/// A raw input line as received from the user, with byte-accurate storage and
/// pre-computed trimming metadata.
///
/// The parser operates on [`Input::trimmed_bytes`] but constructs all
/// diagnostic spans as absolute offsets into the raw byte sequence, so errors
/// are self-contained and render correctly without any adjustment at the
/// call site.
pub struct Input {
    raw: Bytes,
    /// Byte offset of the first non-whitespace byte in `raw`.
    leading_offset: usize,
    /// Byte length of the trimmed content (trailing whitespace excluded).
    trimmed_len: usize,
}

impl Input {
    fn from_raw(raw: Bytes) -> Self {
        let leading_offset = raw.len() - raw.trim_ascii_start().len();
        let trimmed_len = raw.trim_ascii().len();
        Self {
            raw,
            leading_offset,
            trimmed_len,
        }
    }

    /// Construct an [`Input`] from borrowed raw bytes, copying into owned storage.
    ///
    /// Prefer [`Input::from_vec`] or [`Input::from_bytes`] when the caller already
    /// owns the buffer to avoid the extra copy.
    pub fn new(bytes: &[u8]) -> Self {
        Self::from_raw(Bytes::copy_from_slice(bytes))
    }

    /// Construct an [`Input`] from an owned [`Vec<u8>`] without an extra copy.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self::from_raw(Bytes::from(bytes))
    }

    /// Construct an [`Input`] from an owned [`Bytes`] buffer without an extra copy.
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self::from_raw(bytes)
    }

    /// The slice the parser operates on — leading and trailing whitespace removed.
    pub fn trimmed_bytes(&self) -> &[u8] {
        &self.raw[self.leading_offset..self.leading_offset + self.trimmed_len]
    }

    /// Byte offset of the trimmed content within the raw byte sequence.
    pub fn leading_offset(&self) -> usize {
        self.leading_offset
    }

    /// Returns `true` if the input contains only whitespace.
    pub fn is_effectively_empty(&self) -> bool {
        self.trimmed_len == 0
    }

    /// The original bytes as received, including all whitespace.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// A named miette source suitable for embedding in diagnostic variants via
    /// `#[source_code]`. Cloning is cheap — the underlying buffer is reference-counted.
    pub fn as_source(&self) -> InputSource {
        InputSource::new("<input>", self.raw.clone())
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
    fn raw_bytes_preserves_original() {
        let input = Input::new(b"  echo hello\n");
        assert_eq!(input.raw_bytes(), b"  echo hello\n");
    }

    #[test]
    fn non_utf8_bytes_stored_accurately() {
        let bytes = b"echo \xff\xfe";
        let input = Input::new(bytes);
        assert_eq!(input.raw_bytes(), bytes);
        assert_eq!(input.trimmed_bytes(), bytes); // no leading/trailing whitespace
    }
}
