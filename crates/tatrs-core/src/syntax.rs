//! Shared syntax errors with source spans, rendered as caret diagnostics.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub span: Range<usize>,
}

impl SyntaxError {
    pub fn new(message: impl Into<String>, span: Range<usize>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    /// Compiler-style report: the source line, a caret under the offending byte,
    /// then the message. Written to stderr, never stdout.
    pub fn render(&self, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let column = source[..start].chars().count();
        format!(
            "{}\n{}^\n{}\n",
            source.trim_end_matches('\n'),
            " ".repeat(column),
            self.message
        )
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (at byte {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for SyntaxError {}

/// A single token with the byte range it occupied in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub text: String,
    pub span: Range<usize>,
}

/// Split on whitespace, treating `[` and `]` as standalone tokens. This mirrors
/// the shell-friendly grammar: no quoting, brackets are grouping.
pub(crate) fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((start, first)) = chars.next() {
        if first.is_whitespace() {
            continue;
        }
        if first == '[' || first == ']' {
            tokens.push(Token {
                text: first.to_string(),
                span: start..start + 1,
            });
            continue;
        }

        let mut end = start + first.len_utf8();
        while let Some(&(index, ch)) = chars.peek() {
            if ch.is_whitespace() || ch == '[' || ch == ']' {
                break;
            }
            end = index + ch.len_utf8();
            chars.next();
        }
        tokens.push(Token {
            text: source[start..end].to_owned(),
            span: start..end,
        });
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_brackets_and_tags() {
        let tokens = tokenize("[:bug and priority lt 50]");
        let texts: Vec<_> = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["[", ":bug", "and", "priority", "lt", "50", "]"]);
        assert_eq!(tokens[0].span, 0..1);
        assert_eq!(tokens[1].span, 1..5);
    }

    #[test]
    fn caret_points_at_the_span() {
        let error = SyntaxError::new("boom", 4..7);
        let rendered = error.render(":bug and");
        assert!(rendered.contains(":bug and"));
        assert!(rendered.contains("    ^"));
    }
}
