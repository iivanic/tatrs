//! Tatr Update Language: a comma- or whitespace-separated list of actions
//! applied to tasks matched by a query.
//!
//! ```text
//! update ::= action *( ("," | whitespace) action )
//! action ::= 'open' | 'close' | '+' tag | '-' tag
//! ```

use std::ops::Range;

use crate::syntax::SyntaxError;
use crate::task::Task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOp {
    Close,
    Open,
    AddTag(String),
    RemoveTag(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Update {
    ops: Vec<UpdateOp>,
}

impl Update {
    pub fn compile(source: &str) -> Result<Self, SyntaxError> {
        let mut ops = Vec::new();

        for (span, token) in update_tokens(source) {
            match token {
                "close" => ops.push(UpdateOp::Close),
                "open" => ops.push(UpdateOp::Open),
                _ if token.starts_with('+') => {
                    let tag = &token[1..];
                    if tag.is_empty() {
                        return Err(SyntaxError::new("empty tag after `+`", span));
                    }
                    ops.push(UpdateOp::AddTag(tag.to_owned()));
                }
                _ if token.starts_with('-') => {
                    let tag = &token[1..];
                    if tag.is_empty() {
                        return Err(SyntaxError::new("empty tag after `-`", span));
                    }
                    ops.push(UpdateOp::RemoveTag(tag.to_owned()));
                }
                _ => {
                    return Err(SyntaxError::new(
                        format!(
                            "unknown action `{token}`; expected `close`, `open`, `+tag`, or `-tag`"
                        ),
                        span,
                    ));
                }
            }
        }

        Ok(Self { ops })
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn ops(&self) -> &[UpdateOp] {
        &self.ops
    }

    /// Apply all actions. Returns whether anything actually changed, so callers
    /// can avoid rewriting untouched files.
    pub fn apply(&self, task: &mut Task) -> bool {
        let mut changed = false;

        for op in &self.ops {
            match op {
                UpdateOp::Close => {
                    if task.status != "CLOSED" {
                        task.close();
                        changed = true;
                    }
                }
                UpdateOp::Open => {
                    if task.status != "OPEN" {
                        task.reopen();
                        changed = true;
                    }
                }
                UpdateOp::AddTag(tag) => {
                    if !task.tags.iter().any(|existing| existing == tag) {
                        task.tags.push(tag.clone());
                        changed = true;
                    }
                }
                UpdateOp::RemoveTag(tag) => {
                    let before = task.tags.len();
                    task.tags.retain(|existing| existing != tag);
                    if task.tags.len() != before {
                        changed = true;
                    }
                }
            }
        }

        changed
    }

    /// Canonical form for reports: `close,-wip,+agent`.
    pub fn render(&self) -> String {
        self.ops
            .iter()
            .map(|op| match op {
                UpdateOp::Close => "close".to_owned(),
                UpdateOp::Open => "open".to_owned(),
                UpdateOp::AddTag(tag) => format!("+{tag}"),
                UpdateOp::RemoveTag(tag) => format!("-{tag}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn update_tokens(source: &str) -> Vec<(Range<usize>, &str)> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;

    for (index, ch) in source.char_indices() {
        if ch == ',' || ch.is_whitespace() {
            if let Some(begin) = start.take() {
                tokens.push((begin..index, &source[begin..index]));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(begin) = start {
        tokens.push((begin..source.len(), &source[begin..]));
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huid::Huid;

    fn task() -> Task {
        Task::new(
            Huid::parse("20261109-120000").unwrap(),
            "Task",
            vec!["bug".to_owned(), "wip".to_owned()],
            10,
        )
    }

    #[test]
    fn applies_actions_and_reports_change() {
        let update = Update::compile("close,-wip,+agent").unwrap();
        let mut task = task();
        assert!(update.apply(&mut task));
        assert_eq!(task.status, "CLOSED");
        assert_eq!(task.tags, vec!["bug", "agent"]);
        assert!(!update.apply(&mut task), "second application is a no-op");
    }

    #[test]
    fn reopens_and_removes_every_duplicate_tag() {
        let update = Update::compile("open,-bug").unwrap();
        let mut task = task();
        task.tags.push("bug".to_owned());
        assert!(update.apply(&mut task));
        assert_eq!(task.status, "OPEN");
        assert_eq!(task.tags, vec!["wip"]);
    }

    #[test]
    fn whitespace_also_separates_actions() {
        let update = Update::compile("+ui +api close").unwrap();
        assert_eq!(update.len(), 3);
        assert_eq!(update.render(), "+ui,+api,close");
    }

    #[test]
    fn rejects_unknown_or_empty_actions() {
        for bad in ["nope", "+", "-", "close,+"] {
            assert!(Update::compile(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn renders_canonical_form() {
        assert_eq!(
            Update::compile(" close , -wip ").unwrap().render(),
            "close,-wip"
        );
    }
}
