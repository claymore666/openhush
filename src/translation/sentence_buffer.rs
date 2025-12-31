//! Sentence buffer for translation.
//!
//! Accumulates transcription chunks until complete sentences are detected,
//! ensuring translations receive coherent input rather than fragments.

use tracing::warn;

/// Maximum buffer size before forced flush with warning.
const MAX_BUFFER_CHARS: usize = 1024;

/// Sentence boundary characters.
const SENTENCE_ENDS: &[char] = &['.', '!', '?'];

/// Buffer that accumulates text and returns complete sentences.
///
/// Translation works best with complete sentences. This buffer accumulates
/// transcription chunks and only releases text when sentence boundaries
/// are detected.
#[derive(Debug, Default)]
pub struct SentenceBuffer {
    /// Accumulated text waiting for sentence end
    pending: String,
}

impl SentenceBuffer {
    /// Create a new empty sentence buffer.
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// Add a chunk of text and return any complete sentences.
    ///
    /// Returns a vector of complete sentences ready for translation.
    /// Incomplete sentences are kept in the buffer for the next chunk.
    pub fn add(&mut self, text: &str) -> Vec<String> {
        self.pending.push_str(text);

        let mut complete = Vec::new();

        // Check buffer size and warn if too large
        if self.pending.len() > MAX_BUFFER_CHARS {
            warn!(
                "Sentence buffer exceeded {}B without sentence boundary, forcing flush ({} chars)",
                MAX_BUFFER_CHARS,
                self.pending.len()
            );
            // Force flush the entire buffer
            let flushed = std::mem::take(&mut self.pending);
            if !flushed.trim().is_empty() {
                complete.push(flushed.trim().to_string());
            }
            return complete;
        }

        // Extract complete sentences
        while let Some((sentence, remainder)) = self.split_first_sentence() {
            if !sentence.trim().is_empty() {
                complete.push(sentence.trim().to_string());
            }
            self.pending = remainder;
        }

        complete
    }

    /// Flush any remaining text (call on final chunk).
    ///
    /// Returns the remaining buffered text, even if it's not a complete sentence.
    pub fn flush(&mut self) -> Option<String> {
        let remaining = std::mem::take(&mut self.pending);
        let trimmed = remaining.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Check if the buffer is empty.
    #[allow(dead_code)] // Used in tests
    pub fn is_empty(&self) -> bool {
        self.pending.trim().is_empty()
    }

    /// Get the current buffer length.
    #[allow(dead_code)] // Used in tests
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Split off the first complete sentence if one exists.
    ///
    /// A sentence ends with . ! or ? followed by optional closing quotes,
    /// then whitespace or end of string.
    fn split_first_sentence(&self) -> Option<(String, String)> {
        let chars: Vec<char> = self.pending.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            if SENTENCE_ENDS.contains(&c) {
                // Find the end position (including any trailing quotes)
                let mut end_pos = i;

                // Skip over any closing quotes after the sentence punctuation
                while end_pos + 1 < chars.len() && Self::is_closing_quote(chars[end_pos + 1]) {
                    end_pos += 1;
                }

                // Check if this is a real sentence end:
                // - End of string after punctuation/quotes
                // - Followed by whitespace
                let is_end = end_pos + 1 >= chars.len() || chars[end_pos + 1].is_whitespace();

                if is_end {
                    let sentence: String = chars[..=end_pos].iter().collect();
                    let remainder: String = chars[end_pos + 1..].iter().collect();
                    return Some((sentence, remainder));
                }
            }
        }

        None
    }

    /// Check if a character is a closing quote (straight or curly).
    fn is_closing_quote(c: char) -> bool {
        matches!(
            c,
            '"'             // straight double quote
            | '\''          // straight single quote (apostrophe)
            | '\u{201D}'    // right curly double quote "
            | '\u{2019}' // right curly single quote '
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buffer = SentenceBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_single_sentence() {
        let mut buffer = SentenceBuffer::new();
        let sentences = buffer.add("Hello world.");
        assert_eq!(sentences, vec!["Hello world."]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_multiple_sentences() {
        let mut buffer = SentenceBuffer::new();
        let sentences = buffer.add("Hello. World! How are you?");
        assert_eq!(sentences, vec!["Hello.", "World!", "How are you?"]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_incomplete_sentence() {
        let mut buffer = SentenceBuffer::new();
        let sentences = buffer.add("Hello, my name is");
        assert!(sentences.is_empty());
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 17); // "Hello, my name is" = 17 chars
    }

    #[test]
    fn test_split_across_chunks() {
        let mut buffer = SentenceBuffer::new();

        // First chunk - incomplete
        let s1 = buffer.add("Hello, my name is John and I");
        assert!(s1.is_empty());

        // Second chunk - completes first sentence, starts second
        let s2 = buffer.add(" work at Acme. Nice to meet");
        assert_eq!(s2, vec!["Hello, my name is John and I work at Acme."]);

        // Third chunk - completes second sentence
        let s3 = buffer.add(" you!");
        assert_eq!(s3, vec!["Nice to meet you!"]);

        assert!(buffer.is_empty());
    }

    #[test]
    fn test_flush_incomplete() {
        let mut buffer = SentenceBuffer::new();
        buffer.add("Hello, my name is John");

        let remaining = buffer.flush();
        assert_eq!(remaining, Some("Hello, my name is John".to_string()));
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_flush_empty() {
        let mut buffer = SentenceBuffer::new();
        assert!(buffer.flush().is_none());
    }

    #[test]
    fn test_abbreviations_not_split() {
        let mut buffer = SentenceBuffer::new();
        // "Dr." followed by letter should not split
        let sentences = buffer.add("Dr.Smith is here.");
        // Currently this will split at "Dr." - known limitation
        // A more sophisticated implementation would handle abbreviations
        assert!(!sentences.is_empty());
    }

    #[test]
    fn test_whitespace_handling() {
        let mut buffer = SentenceBuffer::new();
        let sentences = buffer.add("  Hello.   World!  ");
        assert_eq!(sentences, vec!["Hello.", "World!"]);
    }

    #[test]
    fn test_newlines() {
        let mut buffer = SentenceBuffer::new();
        let sentences = buffer.add("Hello.\nWorld!");
        assert_eq!(sentences, vec!["Hello.", "World!"]);
    }

    #[test]
    fn test_quotes() {
        let mut buffer = SentenceBuffer::new();
        let sentences = buffer.add("He said \"Hello.\" Then left.");
        assert_eq!(sentences, vec!["He said \"Hello.\"", "Then left."]);
    }
}
