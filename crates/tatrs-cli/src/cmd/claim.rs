use std::time::Duration;

use tatrs_core::claims::ClaimStore;
use tatrs_core::ready::evaluate;
use tatrs_core::{Huid, Repo, time};

use crate::args::{ClaimArgs, Global};
use crate::error::{AppError, map_claim};
use crate::output;

pub fn run(global: &Global, args: &ClaimArgs) -> Result<(), AppError> {
    let repo = Repo::discover(&std::env::current_dir()?)?;
    let store = ClaimStore::locate(&repo);

    if args.status {
        return status(global, &store, args);
    }

    if global.read_only {
        return Err(AppError::Usage(
            "--read-only forbids claiming; use `claim --status`".to_owned(),
        ));
    }

    let raw = args
        .huid
        .as_deref()
        .ok_or_else(|| AppError::Usage("pass a task id, or use `--status`".to_owned()))?;
    let id = Huid::parse(raw).map_err(|err| AppError::Usage(err.to_string()))?;
    let ttl_secs = time::parse_duration(&args.ttl).ok_or_else(|| {
        AppError::Usage(format!(
            "invalid --ttl `{}`; use 30s, 15m, 2h, or 1d",
            args.ttl
        ))
    })?;
    let ttl = Duration::from_secs(ttl_secs);

    let claim = if let Some(token) = &args.token {
        store.refresh(&id, token, ttl).map_err(map_claim)?
    } else {
        let loaded = repo.load();
        output::emit_diagnostics(&loaded.diagnostics, &repo, global.quiet);
        let task = loaded
            .tasks
            .iter()
            .find(|task| task.id == id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("task {id} was not found")))?;

        let readiness = evaluate(&task, &loaded.tasks, None, time::now_secs());
        if !readiness.ready {
            if !global.quiet {
                for blocker in &readiness.blockers {
                    eprintln!("{id}: {blocker}");
                }
            }
            return Err(AppError::Invalid);
        }

        let owner = args
            .owner
            .clone()
            .or_else(|| std::env::var("TATRS_OWNER").ok())
            .or_else(|| std::env::var("USER").ok())
            .or_else(|| std::env::var("USERNAME").ok())
            .unwrap_or_else(|| "local".to_owned());

        store
            .claim(&id, &owner, ttl, args.run.clone())
            .map_err(map_claim)?
    };

    if global.quiet {
        return Ok(());
    }

    if global.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "claimed": true,
                "task": claim.task,
                "owner": claim.owner,
                "token": claim.token,
                "acquired_at": claim.acquired_at,
                "expires_at": claim.expires_at,
                "run": claim.run,
            })
        );
    } else {
        println!(
            "claimed {} by {} until {} (token {})",
            claim.task, claim.owner, claim.expires_at, claim.token
        );
    }

    Ok(())
}

fn status(global: &Global, store: &ClaimStore, args: &ClaimArgs) -> Result<(), AppError> {
    if args.owner.is_some() || args.token.is_some() || args.run.is_some() {
        return Err(AppError::Usage(
            "`--status` does not take --owner, --token, or --run".to_owned(),
        ));
    }

    let filters: Vec<Huid> = args
        .tasks
        .iter()
        .map(|raw| Huid::parse(raw).map_err(|err| AppError::Usage(err.to_string())))
        .collect::<Result<_, _>>()?;

    let now = time::now_secs();
    let mut claims = Vec::new();
    for entry in store.list() {
        match (entry.claim, entry.error) {
            (Some(claim), _) => {
                if filters.is_empty() || filters.iter().any(|id| claim.task == id.as_str()) {
                    claims.push(claim);
                }
            }
            (None, Some(error)) if !global.quiet => {
                eprintln!(
                    "tatrs: corrupt claim file {}: {error}",
                    entry.path.display()
                );
            }
            _ => {}
        }
    }

    claims.sort_by(|left, right| {
        let left_live = left.is_live(now);
        let right_live = right.is_live(now);
        right_live
            .cmp(&left_live)
            .then_with(|| {
                if left_live {
                    left.expires_at.cmp(&right.expires_at)
                } else {
                    right.expires_at.cmp(&left.expires_at)
                }
            })
            .then_with(|| left.task.cmp(&right.task))
    });

    if claims.is_empty() && args.fail_if_empty {
        return Err(AppError::NotFound("no claims".to_owned()));
    }

    if global.quiet {
        return Ok(());
    }

    if global.json {
        let payload: Vec<_> = claims
            .iter()
            .map(|claim| {
                let live = claim.is_live(now);
                serde_json::json!({
                    "task": claim.task,
                    "owner": claim.owner,
                    "state": if live { "live" } else { "expired" },
                    "acquired_at": claim.acquired_at,
                    "expires_at": claim.expires_at,
                    "remaining_seconds": claim.remaining_seconds(now),
                    "host": claim.host,
                    "pid": claim.pid,
                    "run": claim.run,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "schema": tatrs_core::json::SCHEMA_VERSION,
                "claims": payload,
            })
        );
        return Ok(());
    }

    if claims.is_empty() {
        println!("no claims");
        return Ok(());
    }

    let rows: Vec<[String; 6]> = claims
        .iter()
        .map(|claim| {
            let remaining = claim.remaining_seconds(now).unwrap_or(0);
            [
                claim.task.clone(),
                claim.owner.clone(),
                if claim.is_live(now) {
                    "live"
                } else {
                    "expired"
                }
                .to_owned(),
                relative(remaining),
                claim.host.clone(),
                claim.run.clone().unwrap_or_default(),
            ]
        })
        .collect();

    let header = ["TASK", "OWNER", "STATE", "EXPIRES", "HOST", "RUN"];
    let mut widths = header.map(str::len);
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    println!("{}", format_row(&header.map(str::to_owned), &widths));
    for row in &rows {
        println!("{}", format_row(row, &widths));
    }

    Ok(())
}

fn format_row(cells: &[String; 6], widths: &[usize; 6]) -> String {
    (0..6)
        .map(|index| format!("{:<width$}", cells[index], width = widths[index]))
        .collect::<Vec<_>>()
        .join("  ")
        .trim_end()
        .to_owned()
}

fn relative(seconds: i64) -> String {
    if seconds >= 0 {
        format!("in {}", humanize(seconds))
    } else {
        format!("{} ago", humanize(-seconds))
    }
}

fn humanize(seconds: i64) -> String {
    if seconds >= 86_400 {
        let days = seconds / 86_400;
        let hours = (seconds % 86_400) / 3_600;
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if seconds >= 3_600 {
        format!("{}h {:02}m", seconds / 3_600, (seconds % 3_600) / 60)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}
