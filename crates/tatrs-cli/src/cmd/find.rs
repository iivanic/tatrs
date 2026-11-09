use tatrs_core::json;
use tatrs_core::{Huid, Repo};

use crate::args::{FindArgs, Global};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &FindArgs) -> Result<(), AppError> {
    let id = Huid::parse(&args.huid).map_err(|err| AppError::Usage(err.to_string()))?;
    let repo = Repo::discover(&std::env::current_dir()?)?;
    let task = repo.load_task(&id)?;
    output::emit_diagnostics(&task.diagnostics, &repo, global.quiet);

    let rel_path = repo.rel_task_file(&id);

    if args.path_only {
        println!("{rel_path}");
        return Ok(());
    }

    if global.json {
        println!("{}", json::task_json(&task, &rel_path));
        return Ok(());
    }

    if !global.quiet {
        println!("{}", output::task_report(&task, &rel_path));
    }

    Ok(())
}
