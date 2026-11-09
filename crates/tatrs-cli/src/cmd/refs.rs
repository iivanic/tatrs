use tatrs_core::refs::find_references_with;
use tatrs_core::{Huid, Repo};

use crate::args::{Global, RefArgs};
use crate::error::AppError;

pub fn run(global: &Global, args: &RefArgs) -> Result<(), AppError> {
    let cwd = std::env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let id = match &args.huid {
        Some(raw) => Huid::parse(raw).map_err(|err| AppError::Usage(err.to_string()))?,
        None => repo.current_task(&cwd).ok_or_else(|| {
            AppError::Usage(
                "pass a HUID or run `tatrs ref` inside a task directory (tasks/<HUID>/)".to_owned(),
            )
        })?,
    };

    let references = find_references_with(&repo, &id, !args.no_ignore);

    if references.is_empty() && args.fail_if_empty {
        return Err(AppError::NotFound(format!("no references to {id}")));
    }

    if global.quiet {
        return Ok(());
    }

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "huid": id.as_str(),
                "references": references,
            })
        );
        return Ok(());
    }

    for reference in &references {
        println!("{}:{}:{}", reference.path, reference.line, reference.text);
    }

    Ok(())
}
