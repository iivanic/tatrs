use std::collections::BTreeMap;

use tatrs_core::lifecycle::{self, Context};
use tatrs_core::summary::{parse_tag_descriptions, summarize};
use tatrs_core::time;

use crate::args::{Global, SummaryArgs};
use crate::board;
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &SummaryArgs) -> Result<(), AppError> {
    let repo = tatrs_core::Repo::discover(&std::env::current_dir()?)?;
    let loaded = repo.load();
    output::emit_diagnostics(&loaded.diagnostics, &repo, global.quiet);

    let claims = board::load_claims(&repo);
    let activity = board::activity(&loaded.tasks);
    let lifecycles = lifecycle::classify_all(
        &loaded.tasks,
        &claims,
        &activity,
        Context::new(time::now_secs()),
    );
    let board = lifecycle::counts(&lifecycles);

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
                "lifecycle": board,
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

    println!("LIFECYCLE:");
    for (label, value) in [
        ("received", board.received),
        ("in progress", board.in_progress),
        ("paused", board.paused),
        ("waiting", board.waiting),
        ("completed", board.completed),
        ("refused", board.refused),
        ("ready", board.ready),
        ("attention", board.attention),
    ] {
        println!("    {label:<12} {value:>4}");
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
