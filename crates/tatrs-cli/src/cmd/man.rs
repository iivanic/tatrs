use clap::CommandFactory;

use crate::args::{Cli, Global};
use crate::error::AppError;

pub fn run(_global: &Global) -> Result<(), AppError> {
    let command = Cli::command();
    clap_mangen::Man::new(command)
        .render(&mut std::io::stdout())
        .map_err(|err| AppError::Runtime(format!("cannot render man page: {err}")))?;
    Ok(())
}
