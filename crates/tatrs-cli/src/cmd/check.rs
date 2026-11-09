use std::path::Path;

use tatrs_core::json;
use tatrs_core::{Repo, Severity};

use crate::args::{CheckArgs, Global};
use crate::error::AppError;
use crate::output;

pub fn run(global: &Global, args: &CheckArgs) -> Result<(), AppError> {
    let repo = Repo::discover(&std::env::current_dir()?)?;
    let loaded = repo.load();

    let mut diagnostics = loaded.diagnostics;
    for task in &loaded.tasks {
        diagnostics.extend(task.lint());
    }

    let errors = diagnostics.iter().filter(|d| d.is_error()).count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let clean = errors == 0 && (!args.strict || warnings == 0);
    // `--lenient` is migration mode: report everything, but never fail the run.
    let ok = clean || global.lenient;

    if global.json {
        let values: Vec<_> = diagnostics
            .iter()
            .map(|d| d.to_json(Path::new(&repo.rel_path(&d.path))))
            .collect();
        println!("{}", json::check_json(ok, loaded.tasks.len(), &values));
    } else {
        for diagnostic in &diagnostics {
            println!("{}", output::diagnostic_line(diagnostic, &repo));
        }
        if !global.quiet {
            println!(
                "\n{} task(s), {} error(s), {} warning(s)",
                loaded.tasks.len(),
                errors,
                warnings
            );
        }
    }

    if ok { Ok(()) } else { Err(AppError::Invalid) }
}
