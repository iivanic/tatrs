//! Summary counts for `tatrs summary`.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::task::Task;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PriorityCount {
    pub priority: i64,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub status: &'static str,
    pub total: usize,
    pub untagged: usize,
    pub tags: Vec<TagCount>,
    pub priorities: Vec<PriorityCount>,
}

/// Count open (or closed) tasks by tag. Tags are sorted by count descending,
/// then by name for stable output.
pub fn summarize(tasks: &[Task], closed: bool, descriptions: &BTreeMap<String, String>) -> Summary {
    let mut total = 0usize;
    let mut untagged = 0usize;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut priorities: BTreeMap<i64, usize> = BTreeMap::new();

    for task in tasks {
        if task.is_closed() != closed {
            continue;
        }
        total += 1;
        if task.tags.is_empty() {
            untagged += 1;
        }
        for tag in &task.tags {
            *counts.entry(tag.as_str()).or_default() += 1;
        }
        *priorities.entry(task.priority).or_default() += 1;
    }

    let mut tags: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, count)| TagCount {
            tag: tag.to_owned(),
            count,
            description: descriptions.get(tag).cloned().filter(|d| !d.is_empty()),
        })
        .collect();
    tags.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.tag.cmp(&right.tag))
    });

    Summary {
        status: if closed { "CLOSED" } else { "OPEN" },
        total,
        untagged,
        tags,
        priorities: priorities
            .into_iter()
            .rev()
            .map(|(priority, count)| PriorityCount { priority, count })
            .collect(),
    }
}

/// Parse the optional `tasks/tags` file: `<tag> [,] <description>` per line.
/// Malformed lines are skipped; a repeated tag keeps the last description.
pub fn parse_tag_descriptions(source: &str) -> BTreeMap<String, String> {
    let mut descriptions = BTreeMap::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let split = line
            .find(|ch: char| ch == ',' || ch.is_whitespace())
            .unwrap_or(line.len());
        let tag = line[..split].trim();
        if tag.is_empty() {
            continue;
        }
        let description = line[split..]
            .trim_start_matches(|ch: char| ch == ',' || ch.is_whitespace())
            .trim()
            .to_owned();
        descriptions.insert(tag.to_owned(), description);
    }

    descriptions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huid::Huid;

    fn task(tags: &[&str], closed: bool) -> Task {
        let mut task = Task::new(
            Huid::parse("20261109-120000").unwrap(),
            "T",
            tags.iter().map(|t| (*t).to_owned()).collect(),
            10,
        );
        if closed {
            task.close();
        }
        task
    }

    #[test]
    fn counts_tags_for_the_selected_status_only() {
        let tasks = vec![
            task(&["bug", "ui"], false),
            task(&["bug"], false),
            task(&["closed-only"], true),
        ];
        let descriptions =
            parse_tag_descriptions("bug, unintended behavior\nui -> not the syntax\n");
        let summary = summarize(&tasks, false, &descriptions);

        assert_eq!(summary.status, "OPEN");
        assert_eq!(summary.total, 2);
        assert_eq!(summary.untagged, 0);
        assert_eq!(summary.tags[0].tag, "bug");
        assert_eq!(summary.tags[0].count, 2);
        assert_eq!(
            summary.tags[0].description.as_deref(),
            Some("unintended behavior")
        );
        assert_eq!(summary.tags[1].tag, "ui");
        assert_eq!(summary.tags[1].count, 1);
        assert_eq!(
            summary.priorities,
            vec![PriorityCount {
                priority: 10,
                count: 2
            }]
        );
    }

    #[test]
    fn counts_untagged_tasks() {
        let tasks = vec![task(&[], false), task(&["x"], false)];
        let summary = summarize(&tasks, false, &BTreeMap::new());
        assert_eq!(summary.untagged, 1);
    }

    #[test]
    fn descriptions_tolerate_commas_and_spaces() {
        let descriptions = parse_tag_descriptions("a , first\nb,second\nc\n");
        assert_eq!(descriptions.get("a").map(String::as_str), Some("first"));
        assert_eq!(descriptions.get("b").map(String::as_str), Some("second"));
        assert_eq!(descriptions.get("c").map(String::as_str), Some(""));
    }
}
