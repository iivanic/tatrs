use std::collections::HashSet;
use std::time::Duration;

use tatrs_core::{Huid, Query, Repo, SyntaxError, Task, Update, json};

use crate::args::{Global, LsArgs, Sort};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &LsArgs) -> Result<(), AppError> {
    let query_source = args.query.join(" ");
    let has_query = !query_source.trim().is_empty();

    let query = if has_query {
        Query::compile(&query_source).map_err(|err| report_syntax(&err, &query_source))?
    } else {
        Query::any()
    };

    let update = match &args.update {
        Some(source) => {
            let parsed = Update::compile(source).map_err(|err| report_syntax(&err, source))?;
            (!parsed.is_empty()).then_some(parsed)
        }
        None => None,
    };

    if update.is_some() && !has_query {
        return Err(AppError::Usage(
            "`--update` requires a query; use `any` to update every task".to_owned(),
        ));
    }
    if update.is_some() && !args.dry_run && global.read_only {
        return Err(AppError::Usage(
            "--read-only forbids `--update`; use --dry-run to preview".to_owned(),
        ));
    }

    let repo = Repo::discover(&std::env::current_dir()?)?;
    let loaded = repo.load();
    output::emit_diagnostics(&loaded.diagnostics, &repo, global.quiet);

    let mut tasks: Vec<Task> = loaded
        .tasks
        .into_iter()
        .filter(|task| {
            if args.all {
                true
            } else if args.closed {
                task.is_closed()
            } else {
                !task.is_closed()
            }
        })
        .filter(|task| {
            args.tags
                .iter()
                .all(|tag| task.tags.iter().any(|candidate| candidate == tag))
        })
        .filter(|task| query.matches(task))
        .collect();

    let sort = args.sort.unwrap_or(Sort::Priority);
    tasks.sort_by(|left, right| match sort {
        Sort::Priority => right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id)),
        Sort::Id => left.id.cmp(&right.id),
    });

    let mut changed: HashSet<Huid> = HashSet::new();
    if let Some(update) = &update {
        // Rewriting a task whose header could not be parsed would replace its
        // body with a fresh one. Refuse the whole batch instead.
        let malformed = tasks
            .iter()
            .filter(|task| {
                task.diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.is_error())
            })
            .count();
        if malformed > 0 {
            if !global.quiet {
                eprintln!(
                    "tatrs: refusing to update {malformed} malformed task(s); run `tatrs check` first"
                );
            }
            return Err(AppError::Invalid);
        }

        // The lock is held for the whole batch; dry runs need no exclusion.
        let _lock = if args.dry_run {
            None
        } else {
            Some(repo.try_lock(Duration::from_secs(5))?)
        };

        for task in &mut tasks {
            if update.apply(task) {
                changed.insert(task.id.clone());
                if !args.dry_run {
                    repo.save(task)?;
                }
            }
        }
    }

    let matched = tasks.len();
    if matched == 0 && args.fail_if_empty {
        return Err(AppError::NotFound("no tasks matched".to_owned()));
    }

    if global.quiet {
        return Ok(());
    }

    if global.json {
        let mut payload = json::tasks_json(
            tasks
                .iter()
                .map(|task| (task, repo.rel_task_file(&task.id))),
        );
        if let (Some(object), Some(update)) = (payload.as_object_mut(), &update) {
            object.insert("updated".to_owned(), serde_json::json!(changed.len()));
            object.insert("dry_run".to_owned(), serde_json::json!(args.dry_run));
            object.insert("update".to_owned(), serde_json::json!(update.render()));
        }
        println!("{payload}");
        return Ok(());
    }

    if global.porcelain {
        for task in &tasks {
            println!(
                "{}",
                output::porcelain_line(task, &repo.rel_task_file(&task.id))
            );
        }
        return Ok(());
    }

    if tasks.is_empty() {
        println!("No tasks were found");
    } else {
        for task in &tasks {
            let marker = update.as_ref().and_then(|update| {
                changed
                    .contains(&task.id)
                    .then(|| format!("UPDATED: {}", update.render()))
            });
            println!(
                "{}",
                output::task_report_with(task, &repo.rel_task_file(&task.id), marker.as_deref())
            );
        }
    }

    if update.is_some() {
        println!(
            "\n{} task(s) {} ({} matched)",
            changed.len(),
            if args.dry_run {
                "would be updated"
            } else {
                "updated"
            },
            matched
        );
    }

    Ok(())
}

fn report_syntax(error: &SyntaxError, source: &str) -> AppError {
    eprint!("{}", error.render(source));
    AppError::Reported(2)
}
