use clap::CommandFactory;
use clap_complete::generate;

use crate::args::{Cli, CompletionsArgs, Global};
use crate::error::AppError;

pub fn run(_global: &Global, args: &CompletionsArgs) -> Result<(), AppError> {
    let mut command = Cli::command();
    generate(args.shell, &mut command, "tatrs", &mut std::io::stdout());
    Ok(())
}
