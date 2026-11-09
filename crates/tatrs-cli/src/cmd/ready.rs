use std::collections::HashMap;

use tatrs_core::claims::ClaimStore;
use tatrs_core::json;
use tatrs_core::ready::evaluate;
use tatrs_core::{Repo, time};

use crate::args::{Global, ReadyArgs};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &ReadyArgs) -> Result<(), AppError> {
    let repo = Repo::discover(&std::env::current_dir()?)?;
    let loaded = repo.load();
    output::emit_diagnostics(&loaded.diagnostics, &repo, global.quiet);

    let store = ClaimStore::locate(&repo);
    let now = time::now_secs();
    let mut claims = HashMap::new();
    for entry in store.list() {
        match (entry.claim, entry.error) {
            (Some(claim), _) => {
                claims.insert(claim.task.clone(), claim);
            }
            (None, Some(error)) if !global.quiet => {
                eprintln!(
                    "tatrs: corrupt claim file {}: {error}",
                    entry.path.display()
                );
            }
            _ => {}
        }
    }

    let mut ready = Vec::new();
    let mut excluded = Vec::new();
    for task in &loaded.tasks {
        if task.is_closed() {
            continue;
        }
        let readiness = evaluate(task, &loaded.tasks, claims.get(task.id.as_str()), now);
        if readiness.ready {
            ready.push(task);
        } else {
            excluded.push((task, readiness.blockers));
        }
    }

    ready.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
    if let Some(limit) = args.limit {
        ready.truncate(limit);
    }

    if global.quiet {
        return Ok(());
    }

    if global.json {
        let tasks: Vec<_> = ready
            .iter()
            .map(|task| json::task_json(task, &repo.rel_task_file(&task.id)))
            .collect();
        let mut payload = serde_json::json!({
            "schema": json::SCHEMA_VERSION,
            "ready": tasks,
        });
        if args.explain {
            let skipped: Vec<_> = excluded
                .iter()
                .map(|(task, reasons)| {
                    serde_json::json!({
                        "id": task.id.as_str(),
                        "title": task.title,
                        "reasons": reasons,
                    })
                })
                .collect();
            payload["excluded"] = serde_json::Value::Array(skipped);
        }
        println!("{payload}");
        return Ok(());
    }

    if ready.is_empty() {
        println!("No ready tasks");
    } else {
        for task in &ready {
            println!(
                "{}",
                output::task_report(task, &repo.rel_task_file(&task.id))
            );
        }
    }

    if args.explain && !excluded.is_empty() {
        println!("\nnot ready:");
        for (task, reasons) in &excluded {
            println!("  {}: {}", task.id, reasons.join("; "));
        }
    }

    Ok(())
}
