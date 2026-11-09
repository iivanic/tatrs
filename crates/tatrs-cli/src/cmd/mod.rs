//! Command dispatch. Each subcommand is a thin adapter over `tatrs-core`.

pub mod check;
pub mod init;
pub mod ls;
pub mod new;
pub mod show;
pub mod version;

use crate::args::{Cli, Command};
use crate::error::AppError;

pub fn dispatch(cli: Cli) -> Result<(), AppError> {
    let Cli { global, command } = cli;

    if let Some(dir) = &global.cwd {
        std::env::set_current_dir(dir).map_err(|err| {
            AppError::Runtime(format!(
                "cannot change directory to {}: {err}",
                dir.display()
            ))
        })?;
    }

    let is_dry_run = matches!(&command, Command::New(args) if args.dry_run);
    let mutating = matches!(&command, Command::Init | Command::New(_));
    if global.read_only && mutating && !is_dry_run {
        return Err(AppError::Usage(
            "--read-only forbids `init` and `new` (use `new --dry-run` to preview)".to_owned(),
        ));
    }

    match command {
        Command::Init => init::run(&global),
        Command::New(args) => new::run(&global, &args),
        Command::Ls(args) => ls::run(&global, &args),
        Command::Show(args) => show::run(&global, &args),
        Command::Check(args) => check::run(&global, &args),
        Command::Version => version::run(&global),
    }
}
