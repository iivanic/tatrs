use std::fs;

use tatrs_core::json;
use tatrs_core::store::write_atomic;
use tatrs_core::task::split_tags;
use tatrs_core::todo;
use tatrs_core::{NewTask, Repo};

use crate::args::{Global, TodoArgs};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &TodoArgs) -> Result<(), AppError> {
    let write = args.yes && !args.dry_run;
    if write && global.read_only {
        return Err(AppError::Usage(
            "--read-only forbids `todo`; use --dry-run to preview".to_owned(),
        ));
    }

    let repo = Repo::discover(&std::env::current_dir()?)?;
    let tags = split_tags(&args.tags.join(","));

    let mut planned: Vec<(String, usize, String)> = Vec::new();
    let mut created = Vec::new();
    let mut changed_files = Vec::new();

    for file in &args.files {
        let source = fs::read_to_string(file)
            .map_err(|err| AppError::Runtime(format!("cannot read {}: {err}", file.display())))?;
        let todos = todo::scan(&source);
        if todos.is_empty() {
            continue;
        }

        if !write {
            for item in &todos {
                planned.push((file.display().to_string(), item.line, item.title.clone()));
            }
            continue;
        }

        let mut ids = Vec::with_capacity(todos.len());
        for item in &todos {
            let title = if item.title.is_empty() {
                format!("TODO in {}:{}", file.display(), item.line)
            } else {
                item.title.clone()
            };
            let mut spec = NewTask::new(title);
            spec.tags = tags.clone();
            spec.suffix = args.suffix.clone();
            let task = repo.create(&spec)?;
            ids.push(task.id.clone());
            created.push(task);
        }

        let rewritten = todo::rewrite(&source, &ids).ok_or_else(|| {
            AppError::Runtime(format!(
                "TODO markers in {} changed while rewriting; file left untouched",
                file.display()
            ))
        })?;
        write_atomic(file, &rewritten)?;
        changed_files.push(file.display().to_string());
    }

    if global.quiet {
        return Ok(());
    }

    if global.json {
        if write {
            let tasks: Vec<_> = created
                .iter()
                .map(|task| json::task_json(task, &repo.rel_task_file(&task.id)))
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "schema": json::SCHEMA_VERSION,
                    "created": tasks,
                    "files": changed_files,
                })
            );
        } else {
            let todos: Vec<_> = planned
                .iter()
                .map(|(file, line, title)| {
                    serde_json::json!({ "file": file, "line": line, "title": title })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "schema": json::SCHEMA_VERSION,
                    "dry_run": true,
                    "todos": todos,
                })
            );
        }
        return Ok(());
    }

    if write {
        for task in &created {
            println!(
                "{}",
                output::task_report(task, &repo.rel_task_file(&task.id))
            );
        }
        println!(
            "\n{} task(s) created in {} file(s)",
            created.len(),
            changed_files.len()
        );
    } else {
        for (file, line, title) in &planned {
            println!("{file}:{line}: TODO: {title}");
        }
        println!(
            "would create {} task(s); re-run with --yes to write",
            planned.len()
        );
    }

    Ok(())
}
