mod args;
mod board;
mod cmd;
mod error;
mod output;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = args::Cli::parse();
    match cmd::dispatch(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if let Some(message) = err.message() {
                eprintln!("tatrs: {message}");
            }
            ExitCode::from(err.exit_code())
        }
    }
}
