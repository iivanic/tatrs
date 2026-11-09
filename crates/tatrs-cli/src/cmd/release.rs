use tatrs_core::claims::ClaimStore;
use tatrs_core::{Huid, Repo};

use crate::args::{Global, ReleaseArgs};
use crate::error::{AppError, map_claim};

pub fn run(global: &Global, args: &ReleaseArgs) -> Result<(), AppError> {
    if global.read_only {
        return Err(AppError::Usage(
            "--read-only forbids releasing a claim".to_owned(),
        ));
    }

    let repo = Repo::discover(&std::env::current_dir()?)?;
    let id = Huid::parse(&args.huid).map_err(|err| AppError::Usage(err.to_string()))?;
    let store = ClaimStore::locate(&repo);

    store.release(&id, &args.token).map_err(map_claim)?;

    if global.quiet {
        return Ok(());
    }

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "released": true,
                "task": id.as_str(),
            })
        );
    } else {
        println!("released {id}");
    }

    Ok(())
}
