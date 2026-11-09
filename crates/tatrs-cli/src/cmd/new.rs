use std::io::Read;

use tatrs_core::json;
use tatrs_core::task::{DEFAULT_TITLE, split_tags};
use tatrs_core::{Huid, NewTask, Repo, StoreError};

use crate::args::{Global, NewArgs};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &NewArgs) -> Result<(), AppError> {
    let title = if args.title.is_empty() {
        DEFAULT_TITLE.to_owned()
    } else {
        args.title.join(" ")
    };
    let tags = split_tags(&args.tags.join(","));

    let mut spec = NewTask::new(title);
    spec.tags = tags;
    spec.priority = args.priority;
    spec.suffix = args.suffix.clone();
    spec.body = task_body(args)?;
    if let Some(raw) = &args.id {
        spec.id = Some(Huid::parse(raw).map_err(|err| AppError::Usage(err.to_string()))?);
    }

    if args.dry_run {
        if global.quiet {
            return Ok(());
        }
        if global.json {
            println!(
                "{}",
                serde_json::json!({
                    "schema": json::SCHEMA_VERSION,
                    "dry_run": true,
                    "title": spec.title,
                    "priority": spec.priority,
                    "tags": spec.tags,
                    "id": spec.id.as_ref().map(Huid::as_str),
                })
            );
        } else {
            println!(
                "would create: {} (priority {}, tags: {})",
                spec.title,
                spec.priority,
                if spec.tags.is_empty() {
                    "none".to_owned()
                } else {
                    spec.tags.join(",")
                }
            );
        }
        return Ok(());
    }

    let repo = Repo::discover(&std::env::current_dir()?)?;
    let task = match repo.create(&spec) {
        Ok(task) => task,
        // Deterministic IDs are an idempotency key: a retried agent must not
        // create a duplicate task, and must still learn the task's state.
        Err(StoreError::Exists(id)) if spec.id.is_some() => {
            let task = repo.load_task(&id)?;
            if !global.quiet {
                eprintln!("tatrs: task {id} already exists; nothing to do");
            }
            task
        }
        Err(err) => return Err(err.into()),
    };

    if global.quiet {
        return Ok(());
    }

    let rel_path = repo.rel_task_file(&task.id);
    if global.json {
        println!("{}", json::task_json(&task, &rel_path));
    } else if args.print_id {
        println!("{}", task.id);
    } else {
        println!("{}", output::task_report(&task, &rel_path));
    }

    // `-y` is accepted for upstream compatibility; no command prompts yet.
    let _ = args.yes;
    Ok(())
}

/// Body from `--body`, `--body-file`, `--acceptance`, or empty. A single blank
/// line is guaranteed between the property block and the body.
fn task_body(args: &NewArgs) -> Result<String, AppError> {
    let raw = if let Some(path) = &args.body_file {
        if path.as_os_str() == "-" {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|err| AppError::Usage(format!("cannot read body from stdin: {err}")))?;
            buffer
        } else {
            std::fs::read_to_string(path).map_err(|err| {
                AppError::Usage(format!("cannot read body file {}: {err}", path.display()))
            })?
        }
    } else if let Some(text) = &args.body {
        text.clone()
    } else if args.acceptance {
        "## Acceptance\n\n- [ ] ".to_owned()
    } else {
        String::new()
    };

    // The format is LF and BOM-free; normalize at the write boundary so callers
    // can pipe in files produced on any platform.
    let mut body = raw
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    if !body.is_empty() {
        if !body.starts_with('\n') {
            body.insert(0, '\n');
        }
        if !body.ends_with('\n') {
            body.push('\n');
        }
    }

    Ok(body)
}
