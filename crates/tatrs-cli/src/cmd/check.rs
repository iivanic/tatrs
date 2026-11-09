use std::path::Path;

use tatrs_core::claims::ClaimStore;
use tatrs_core::diagnostics::codes;
use tatrs_core::json;
use tatrs_core::{Diagnostic, Huid, Repo, Severity, time};

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

    let now = time::now_secs();
    let store = ClaimStore::locate(&repo);

    if args.fix {
        if global.read_only {
            return Err(AppError::Usage(
                "--read-only forbids `check --fix`".to_owned(),
            ));
        }
        let mut removed = 0usize;
        for entry in store.list() {
            let remove = match (&entry.claim, &entry.error) {
                (None, _) => true,
                (Some(claim), _) => {
                    let expired_too_long = time::parse_iso(&claim.expires_at)
                        .is_some_and(|expires| expires + 86_400 < now);
                    match Huid::parse(&claim.task) {
                        Ok(id) => match loaded.tasks.iter().find(|task| task.id == id) {
                            None => true,
                            Some(task) if task.is_closed() => true,
                            Some(_) => expired_too_long,
                        },
                        Err(_) => true,
                    }
                }
            };
            if remove && std::fs::remove_file(&entry.path).is_ok() {
                removed += 1;
            }
        }
        if !global.quiet && removed > 0 {
            eprintln!("removed {removed} claim file(s)");
        }
    }

    for entry in store.list() {
        let path = entry.path.clone();
        if let Some(error) = entry.error {
            diagnostics.push(Diagnostic::warning(
                codes::CLAIM_CORRUPT,
                &path,
                None,
                format!("corrupt claim file: {error}"),
            ));
            continue;
        }
        let Some(claim) = entry.claim else {
            continue;
        };

        match Huid::parse(&claim.task) {
            Ok(id) => match loaded.tasks.iter().find(|task| task.id == id) {
                None => diagnostics.push(Diagnostic::warning(
                    codes::CLAIM_ORPHANED,
                    &path,
                    None,
                    format!("claim refers to unknown task {id}"),
                )),
                Some(task) if task.is_closed() => diagnostics.push(Diagnostic::warning(
                    codes::CLAIM_ORPHANED,
                    &path,
                    None,
                    format!("claim refers to closed task {id}"),
                )),
                Some(_) => {}
            },
            Err(_) => diagnostics.push(Diagnostic::warning(
                codes::CLAIM_ORPHANED,
                &path,
                None,
                "claim file has an invalid task id",
            )),
        }

        if let Some(expires) = time::parse_iso(&claim.expires_at)
            && expires + 86_400 < now
        {
            diagnostics.push(Diagnostic::warning(
                codes::CLAIM_STALE,
                &path,
                None,
                format!("claim expired more than a day ago ({})", claim.expires_at),
            ));
        }
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
