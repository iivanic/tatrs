//! `TASK.md` parsing and rendering.
//!
//! The grammar is intentionally mirrored from upstream `tatr`, including its
//! quirks: property keys are alphanumeric only, the property block ends at the
//! first line that is not `- KEY: value`, and the body is everything that
//! follows, preserved verbatim.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::diagnostics::{Diagnostic, Severity, codes};
use crate::huid::Huid;
use crate::props::Properties;

/// Used when `new` is called without a title, matching upstream.
pub const DEFAULT_TITLE: &str = "New Task";
/// Priority given to tasks created by `new`, matching upstream.
pub const DEFAULT_PRIORITY: i64 = 100;
/// Priority materialized for files that do not declare one (upstream behavior).
pub const UNPRIORITIZED: i64 = 999_999;
/// Title reported for files whose first line is not a heading.
pub const INVALID_TITLE_SENTINEL: &str = "!!! INVALID: TASK TITLE MUST START WITH # !!!";

#[derive(Debug, Clone)]
pub struct Task {
    pub id: Huid,
    /// Path to this task's `TASK.md`.
    pub path: PathBuf,
    pub title: String,
    /// Raw status string; only `OPEN` and `CLOSED` are valid, but unknown values
    /// are preserved so `ls` can still show them.
    pub status: String,
    pub priority: i64,
    pub tags: Vec<String>,
    pub props: Properties,
    /// Everything after the property block, verbatim. Usually starts with a
    /// blank line, which is how the blank line before the description survives
    /// a rewrite.
    pub body: String,
    /// Parse-time findings (duplicate/malformed properties, bad title).
    pub diagnostics: Vec<Diagnostic>,
}

impl Task {
    /// Create a task in memory; not yet written.
    pub fn new(id: Huid, title: impl Into<String>, tags: Vec<String>, priority: i64) -> Self {
        let mut props = Properties::new();
        props.put("STATUS", "OPEN");
        props.put("PRIORITY", priority.to_string());
        props.put("TAGS", tags.join(","));
        let title = title.into();
        Self {
            path: PathBuf::new(),
            id,
            title,
            status: "OPEN".to_owned(),
            priority,
            tags,
            props,
            body: String::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Parse `content`. Never fails: structural problems become diagnostics so
    /// that `ls` and `show` keep working on a damaged tree, exactly like
    /// upstream, while `check` reports the damage.
    pub fn parse(id: Huid, content: &str, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        // A UTF-8 BOM before the title would otherwise read as a missing `#`.
        let content = content.strip_prefix('\u{feff}').unwrap_or(content);
        let normalized = content.replace("\r\n", "\n");
        let mut parser = Parser::new(&normalized, path.clone());
        let (title, title_ok) = parser.parse_title();

        if !title_ok {
            // Upstream stops after a missing title: no properties, no body.
            let mut task = Self::defaults(id, path, title);
            task.diagnostics = parser.diagnostics;
            return task;
        }

        let mut props = parser.parse_properties();
        let body = parser.source[parser.pos..].to_owned();
        let mut diagnostics = parser.diagnostics;

        let status = match props.get("STATUS") {
            Some(value) => value.to_owned(),
            None => {
                props.put("STATUS", "OPEN");
                "OPEN".to_owned()
            }
        };

        let priority = match props.get("PRIORITY") {
            Some(value) => match value.trim().parse::<i64>() {
                Ok(priority) => priority,
                Err(_) => {
                    diagnostics.push(Diagnostic::warning(
                        codes::INVALID_PRIORITY,
                        &path,
                        find_property_line(&normalized, "PRIORITY"),
                        format!("priority `{value}` is not an integer; treated as 0"),
                    ));
                    0
                }
            },
            None => {
                props.put("PRIORITY", UNPRIORITIZED.to_string());
                UNPRIORITIZED
            }
        };

        let tags = props.get("TAGS").map(split_tags).unwrap_or_default();
        if !props.contains("TAGS") {
            props.put("TAGS", "");
        }

        Self {
            id,
            path,
            title,
            status,
            priority,
            tags,
            props,
            body,
            diagnostics,
        }
    }

    fn defaults(id: Huid, path: PathBuf, title: String) -> Self {
        let mut props = Properties::new();
        props.put("STATUS", "OPEN");
        props.put("PRIORITY", UNPRIORITIZED.to_string());
        props.put("TAGS", "");
        Self {
            id,
            path,
            title,
            status: "OPEN".to_owned(),
            priority: UNPRIORITIZED,
            tags: Vec::new(),
            props,
            body: String::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.status == "CLOSED"
    }

    pub fn close(&mut self) {
        self.status = "CLOSED".to_owned();
    }

    pub fn reopen(&mut self) {
        self.status = "OPEN".to_owned();
    }

    /// A task can only terminate an agent loop if it states how it is verified.
    pub fn has_acceptance(&self) -> bool {
        self.body.lines().any(|line| {
            line.trim_start()
                .strip_prefix("##")
                .map(|rest| {
                    rest.trim_start()
                        .to_ascii_lowercase()
                        .starts_with("acceptance")
                })
                .unwrap_or(false)
        })
    }

    /// Lint rules that are not parse errors.
    pub fn lint(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();

        if self.status != "OPEN" && self.status != "CLOSED" {
            out.push(Diagnostic::error(
                codes::INVALID_STATUS,
                &self.path,
                None,
                format!(
                    "status `{}` is invalid; use OPEN or CLOSED and express lifecycle in tags",
                    self.status
                ),
            ));
        }

        if !self.is_closed() && !self.has_acceptance() {
            out.push(Diagnostic::warning(
                codes::MISSING_ACCEPTANCE,
                &self.path,
                None,
                "no `## Acceptance` section; agents have no verifiable exit condition",
            ));
        }

        let mut seen = HashSet::new();
        for tag in &self.tags {
            if tag.contains('[') || tag.contains(']') {
                out.push(Diagnostic::warning(
                    codes::TAG_WITH_BRACKETS,
                    &self.path,
                    None,
                    format!("tag `{tag}` contains square brackets, which TQL cannot express"),
                ));
            }
            if !seen.insert(tag.as_str()) {
                out.push(Diagnostic::warning(
                    codes::DUPLICATE_TAG,
                    &self.path,
                    None,
                    format!("tag `{tag}` is listed more than once"),
                ));
            }
        }

        out
    }

    /// Canonical rendering. Body bytes are appended verbatim; only the header is
    /// normalized (LF line endings, one blank line after the title).
    pub fn render(&self) -> String {
        let mut props = self.props.clone();
        props.put("STATUS", self.status.clone());
        props.put("PRIORITY", self.priority.to_string());
        props.put("TAGS", self.tags.join(","));

        let mut out =
            String::with_capacity(self.body.len() + self.title.len() + 16 * props.len() + 16);
        out.push_str("# ");
        out.push_str(self.title.trim_end());
        out.push('\n');
        out.push('\n');

        for property in props.iter() {
            out.push_str("- ");
            out.push_str(&property.key);
            out.push(':');
            let value = property.value.trim();
            if !value.is_empty() {
                out.push(' ');
                out.push_str(value);
            }
            out.push('\n');
        }

        out.push_str(&self.body);
        out
    }
}

/// Split a `TAGS` value on commas and whitespace, dropping empty segments.
pub fn split_tags(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn find_property_line(content: &str, key: &str) -> Option<usize> {
    content.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix('-')?.trim_start();
        let (name, _) = rest.split_once(':')?;
        (name.trim() == key).then_some(index + 1)
    })
}

struct Parser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
    path: PathBuf,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, path: PathBuf) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            path,
            diagnostics: Vec::new(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn line(&self) -> usize {
        self.source[..self.pos]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b) if b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(b) if b.is_ascii_whitespace() && b != b'\n') {
            self.pos += 1;
        }
    }

    fn parse_title(&mut self) -> (String, bool) {
        self.skip_whitespace();
        if self.peek() != Some(b'#') {
            let line = self.line();
            self.diagnostics.push(Diagnostic::error(
                codes::INVALID_TITLE,
                &self.path,
                Some(line),
                "task title must start with `#`",
            ));
            return (INVALID_TITLE_SENTINEL.to_owned(), false);
        }
        self.pos += 1;
        self.skip_spaces();
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if byte == b'\n' {
                break;
            }
            self.pos += 1;
        }
        let title = self.source[start..self.pos].trim().to_owned();
        if self.peek() == Some(b'\n') {
            self.pos += 1;
        }
        (title, true)
    }

    fn parse_properties(&mut self) -> Properties {
        let mut props = Properties::new();
        let mut seen: HashSet<String> = HashSet::new();

        self.skip_whitespace();

        loop {
            let saved = self.pos;
            let line = self.line();
            self.skip_spaces();

            if self.peek() != Some(b'-') {
                self.pos = saved;
                break;
            }
            self.pos += 1;
            self.skip_spaces();

            let key_start = self.pos;
            while matches!(self.peek(), Some(b) if b.is_ascii_alphanumeric()) {
                self.pos += 1;
            }
            let key = self.source[key_start..self.pos].to_owned();
            self.skip_spaces();

            if key.is_empty() || self.peek() != Some(b':') {
                self.pos = saved;
                let line_end = self.source[saved..]
                    .find('\n')
                    .map(|offset| saved + offset)
                    .unwrap_or(self.source.len());
                let candidate = &self.source[saved..line_end];
                if looks_like_property(candidate) {
                    self.diagnostics.push(Diagnostic::warning(
                        codes::MALFORMED_PROPERTY,
                        &self.path,
                        Some(line),
                        format!(
                            "`{}` looks like a property but its key is not alphanumeric; the line starts the body",
                            candidate.trim()
                        ),
                    ));
                }
                break;
            }
            self.pos += 1;

            let value_start = self.pos;
            while let Some(byte) = self.peek() {
                if byte == b'\n' {
                    break;
                }
                self.pos += 1;
            }
            let value = self.source[value_start..self.pos].trim().to_owned();

            if !seen.insert(key.clone()) {
                self.diagnostics.push(Diagnostic::warning(
                    codes::DUPLICATE_PROPERTY,
                    &self.path,
                    Some(line),
                    format!("property `{key}` is declared more than once; the last value wins"),
                ));
            }
            props.put(key, value);

            if self.peek() == Some(b'\n') {
                self.pos += 1;
            }
        }

        props
    }
}

/// A line that was meant as a property but cannot be one under the grammar.
fn looks_like_property(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('-') else {
        return false;
    };
    let rest = rest.trim_start();
    let Some((key, _)) = rest.split_once(':') else {
        return false;
    };
    let key = key.trim();
    if key.is_empty() {
        return false;
    }
    let mut interesting = false;
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() {
            continue;
        }
        if ch == '-' || ch == '_' || ch == '.' {
            interesting = true;
            continue;
        }
        return false;
    }
    interesting
}

/// Parse-time severity policy: structural problems are errors, style issues are
/// warnings. Kept here so callers do not re-implement the mapping.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(content: &str) -> Task {
        Task::parse(
            Huid::parse("20261109-120000").unwrap(),
            content,
            "tasks/20261109-120000/TASK.md",
        )
    }

    const CANONICAL: &str = concat!(
        "# Fix flaky parser test\n",
        "\n",
        "- STATUS: OPEN\n",
        "- PRIORITY: 10\n",
        "- TAGS: bug,agent\n",
        "- OWNER: agent-3\n",
        "\n",
        "Body line one.\n",
        "\n",
        "## Acceptance\n",
        "\n",
        "- [ ] test passes\n",
    );

    #[test]
    fn parses_canonical_task() {
        let task = parse(CANONICAL);
        assert_eq!(task.title, "Fix flaky parser test");
        assert_eq!(task.status, "OPEN");
        assert_eq!(task.priority, 10);
        assert_eq!(task.tags, vec!["bug", "agent"]);
        assert_eq!(task.props.get("OWNER"), Some("agent-3"));
        assert_eq!(
            task.body,
            "\nBody line one.\n\n## Acceptance\n\n- [ ] test passes\n"
        );
        assert!(task.has_acceptance());
        assert!(task.diagnostics.is_empty());
    }

    #[test]
    fn render_round_trips_canonical_task() {
        assert_eq!(parse(CANONICAL).render(), CANONICAL);
    }

    #[test]
    fn missing_properties_are_materialized() {
        let task = parse("# Bare task\n");
        assert_eq!(task.status, "OPEN");
        assert_eq!(task.priority, UNPRIORITIZED);
        assert!(task.tags.is_empty());
        assert!(task.body.is_empty());
    }

    #[test]
    fn crlf_is_tolerated_and_normalized() {
        let task = parse("# Windows task\r\n\r\n- STATUS: OPEN\r\n- PRIORITY: 1\r\n- TAGS: a\r\n");
        assert_eq!(task.title, "Windows task");
        assert_eq!(task.priority, 1);
        assert_eq!(task.tags, vec!["a"]);
        assert!(!task.render().contains('\r'));
    }

    #[test]
    fn duplicate_properties_warn_and_last_wins() {
        let task = parse("# T\n\n- STATUS: OPEN\n- STATUS: CLOSED\n");
        assert_eq!(task.status, "CLOSED");
        assert!(
            task.diagnostics
                .iter()
                .any(|d| d.code == codes::DUPLICATE_PROPERTY)
        );
    }

    #[test]
    fn missing_title_is_an_error_with_sentinel() {
        let task = parse("Not a heading\n\nbody\n");
        assert_eq!(task.title, INVALID_TITLE_SENTINEL);
        assert!(
            task.diagnostics
                .iter()
                .any(|d| d.code == codes::INVALID_TITLE)
        );
        assert!(task.body.is_empty());
    }

    #[test]
    fn malformed_property_key_starts_the_body() {
        let task = parse("# T\n\n- DEPENDS-ON: 20260101-000000\n\ntext\n");
        assert_eq!(task.props.get("DEPENDS-ON"), None);
        assert!(task.body.starts_with("- DEPENDS-ON"));
        assert!(
            task.diagnostics
                .iter()
                .any(|d| d.code == codes::MALFORMED_PROPERTY)
        );
    }

    #[test]
    fn tags_split_on_commas_and_whitespace() {
        assert_eq!(
            split_tags("foo,,, hello  world"),
            vec!["foo", "hello", "world"]
        );
    }

    #[test]
    fn unknown_properties_survive_a_rewrite() {
        let source = "# T\n\n- STATUS: OPEN\n- PRIORITY: 1\n- TAGS:\n- CUSTOM: value\n\nbody\n";
        assert_eq!(parse(source).render(), source);
    }

    #[test]
    fn body_is_preserved_verbatim() {
        let source = "# T\n\n- STATUS: OPEN\n- PRIORITY: 1\n- TAGS:\n\n  indented\n\n\n- list\n";
        assert_eq!(parse(source).render(), source);
    }

    #[test]
    fn utf8_bom_is_tolerated() {
        let task = parse("\u{feff}# T\n\n- STATUS: OPEN\n- PRIORITY: 1\n- TAGS:\n");
        assert_eq!(task.title, "T");
        assert!(task.diagnostics.is_empty());
    }

    #[test]
    fn lint_flags_invalid_status_and_missing_acceptance() {
        let task = parse("# T\n\n- STATUS: DONE\n- PRIORITY: 1\n- TAGS: a,a\n");
        let codes: Vec<_> = task.lint().into_iter().map(|d| d.code).collect();
        assert!(codes.contains(&codes::INVALID_STATUS));
        assert!(codes.contains(&codes::MISSING_ACCEPTANCE));
        assert!(codes.contains(&codes::DUPLICATE_TAG));
    }

    #[test]
    fn closed_tasks_do_not_need_acceptance() {
        let task = parse("# T\n\n- STATUS: CLOSED\n- PRIORITY: 1\n- TAGS:\n");
        assert!(task.lint().is_empty());
    }
}
