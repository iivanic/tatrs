use crate::args::Global;
use crate::error::AppError;

pub fn run(global: &Global) -> Result<(), AppError> {
    let cwd = std::env::current_dir()?;
    let outcome = tatrs_core::store::init(&cwd)?;

    if global.quiet {
        return Ok(());
    }

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "tasks_dir": "tasks",
                "created": outcome.created,
            })
        );
    } else if outcome.created {
        println!("initialized {}", outcome.tasks_dir.display());
    } else {
        println!("tasks/ already exists in {}", cwd.display());
    }

    Ok(())
}
