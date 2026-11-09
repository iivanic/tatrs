use tatrs_core::json;
use tatrs_core::{Repo, Task};

use crate::args::{Global, LsArgs, Sort};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &LsArgs) -> Result<(), AppError> {
    let repo = Repo::discover(&std::env::current_dir()?)?;
    let loaded = repo.load();
    output::emit_diagnostics(&loaded.diagnostics, &repo, global.quiet);

    let mut tasks: Vec<&Task> = loaded
        .tasks
        .iter()
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
        .collect();

    let sort = args.sort.unwrap_or(Sort::Priority);
    tasks.sort_by(|left, right| match sort {
        Sort::Priority => right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id)),
        Sort::Id => left.id.cmp(&right.id),
    });

    if tasks.is_empty() && args.fail_if_empty {
        return Err(AppError::NotFound("no tasks matched".to_owned()));
    }

    if global.quiet {
        return Ok(());
    }

    if global.json {
        println!(
            "{}",
            json::tasks_json(
                tasks
                    .iter()
                    .map(|task| (*task, repo.rel_task_file(&task.id)))
            )
        );
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
        return Ok(());
    }

    for task in &tasks {
        println!(
            "{}",
            output::task_report(task, &repo.rel_task_file(&task.id))
        );
    }

    Ok(())
}
