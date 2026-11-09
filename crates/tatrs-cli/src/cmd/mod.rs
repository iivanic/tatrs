//! Command dispatch. Each subcommand is a thin adapter over `tatrs-core`.

pub mod check;
pub mod claim;
pub mod find;
pub mod id;
pub mod init;
pub mod ls;
pub mod mcp;
pub mod new;
pub mod ready;
pub mod refs;
pub mod release;
pub mod show;
pub mod summary;
pub mod todo;
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
        Command::Find(args) => find::run(&global, &args),
        Command::Id => id::run(&global),
        Command::Ref(args) => refs::run(&global, &args),
        Command::Summary(args) => summary::run(&global, &args),
        Command::Todo(args) => todo::run(&global, &args),
        Command::Ready(args) => ready::run(&global, &args),
        Command::Claim(args) => claim::run(&global, &args),
        Command::Release(args) => release::run(&global, &args),
        Command::Mcp(args) => mcp::run(&global, &args),
        Command::Check(args) => check::run(&global, &args),
        Command::Version => version::run(&global),
    }
}
