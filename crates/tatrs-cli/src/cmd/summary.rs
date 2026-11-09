use std::collections::BTreeMap;

use tatrs_core::summary::{parse_tag_descriptions, summarize};

use crate::args::{Global, SummaryArgs};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &SummaryArgs) -> Result<(), AppError> {
    let repo = tatrs_core::Repo::discover(&std::env::current_dir()?)?;
    let loaded = repo.load();
    output::emit_diagnostics(&loaded.diagnostics, &repo, global.quiet);

    let descriptions = std::fs::read_to_string(repo.tasks_dir().join("tags"))
        .map(|source| parse_tag_descriptions(&source))
        .unwrap_or_else(|_| BTreeMap::new());

    let summary = summarize(&loaded.tasks, args.closed, &descriptions);

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "status": summary.status,
                "total": summary.total,
                "untagged": summary.untagged,
                "tags": summary.tags,
                "priorities": summary.priorities,
            })
        );
        return Ok(());
    }

    if global.quiet {
        return Ok(());
    }

    println!("STATUS:   {}", summary.status);
    println!("TOTAL:    {}", summary.total);
    if summary.untagged > 0 {
        println!("UNTAGGED: {}", summary.untagged);
    }
    if !summary.tags.is_empty() {
        println!("TAGGED:");
        let width = summary
            .tags
            .iter()
            .map(|tag| tag.tag.chars().count())
            .max()
            .unwrap_or(0);
        for tag in &summary.tags {
            match &tag.description {
                Some(description) => println!(
                    "    {:>width$} => {:>3} - {}",
                    tag.tag,
                    tag.count,
                    description,
                    width = width
                ),
                None => println!("    {:>width$} => {:>3}", tag.tag, tag.count, width = width),
            }
        }
    }

    Ok(())
}
