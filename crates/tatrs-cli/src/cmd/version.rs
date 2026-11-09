use crate::args::Global;
use crate::error::AppError;

pub fn run(global: &Global) -> Result<(), AppError> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const COMMIT: Option<&str> = option_env!("TATRS_GIT_COMMIT");

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "name": "tatrs",
                "version": VERSION,
                "commit": COMMIT,
            })
        );
    } else {
        println!("tatrs {VERSION}");
        if let Some(commit) = COMMIT {
            println!("commit {commit}");
        }
        println!("json schema {}", tatrs_core::json::SCHEMA_VERSION);
    }

    Ok(())
}
