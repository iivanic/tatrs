use tatrs_core::Repo;

use crate::args::Global;
use crate::error::AppError;

pub fn run(global: &Global) -> Result<(), AppError> {
    let cwd = std::env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let id = repo.current_task(&cwd).ok_or_else(|| {
        AppError::Usage("run `tatrs id` inside a task directory (tasks/<HUID>/)".to_owned())
    })?;

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "id": id.as_str(),
            })
        );
    } else {
        println!("{id}");
    }

    Ok(())
}
