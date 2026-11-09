//! Conversion of `TODO:` comments into tasks.
//!
//! Only the first marker on a line is converted, matching upstream. The rest of
//! the line is preserved byte-for-byte, so `TODO: x` becomes `TASK(<id>): x`
//! without touching indentation, suffixes, or line endings.

use serde::Serialize;

use crate::huid::Huid;

pub const MARKER: &str = "TODO:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Todo {
    /// 1-based line number.
    pub line: usize,
    /// Byte offset of the marker in the source.
    pub offset: usize,
    /// Trimmed text after the marker; may be empty.
    pub title: String,
}

pub fn scan(source: &str) -> Vec<Todo> {
    let mut todos = Vec::new();
    let mut offset = 0usize;

    for (index, line) in source.split_inclusive('\n').enumerate() {
        if let Some(position) = line.find(MARKER) {
            let title = line[position + MARKER.len()..].trim().to_owned();
            todos.push(Todo {
                line: index + 1,
                offset: offset + position,
                title,
            });
        }
        offset += line.len();
    }

    todos
}

/// Replace each marker with `TASK(<id>):`. Returns `None` when the number of
/// IDs does not match the number of scanned markers, which means the file
/// changed underneath the caller.
pub fn rewrite(source: &str, ids: &[Huid]) -> Option<String> {
    let todos = scan(source);
    if todos.len() != ids.len() {
        return None;
    }

    let mut out = String::with_capacity(source.len() + ids.len() * 24);
    let mut cursor = 0usize;
    for (todo, id) in todos.iter().zip(ids) {
        out.push_str(&source[cursor..todo.offset]);
        out.push_str("TASK(");
        out.push_str(id.as_str());
        out.push_str("):");
        cursor = todo.offset + MARKER.len();
    }
    out.push_str(&source[cursor..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_lines_and_titles() {
        let source = "fn main() {}\n// TODO: fix the parser\n// TODO:   \n";
        let todos = scan(source);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].line, 2);
        assert_eq!(todos[0].title, "fix the parser");
        assert_eq!(todos[1].title, "");
    }

    #[test]
    fn only_the_first_marker_per_line_is_converted() {
        let source = "// TODO: one TODO: two\n";
        assert_eq!(scan(source).len(), 1);
    }

    #[test]
    fn rewrite_preserves_prefix_suffix_and_crlf() {
        let source = "int x; // TODO: add docs\r\nafter\r\n";
        let ids = vec![Huid::parse("20261109-120000").unwrap()];
        let rewritten = rewrite(source, &ids).unwrap();
        assert_eq!(
            rewritten,
            "int x; // TASK(20261109-120000): add docs\r\nafter\r\n"
        );
    }

    #[test]
    fn rewrite_rejects_a_count_mismatch() {
        let source = "TODO: a\nTODO: b\n";
        let ids = vec![Huid::parse("20261109-120000").unwrap()];
        assert!(rewrite(source, &ids).is_none());
    }
}
