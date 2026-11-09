use std::io::BufWriter;

use tatrs_core::Repo;
use tatrs_mcp::Server;

use crate::args::{Global, McpArgs};
use crate::error::AppError;

pub fn run(global: &Global, args: &McpArgs) -> Result<(), AppError> {
    let repo = Repo::discover(&std::env::current_dir()?)?;
    let allow_write = args.allow_write && !global.read_only;
    let server = Server::new(repo, allow_write);

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut output = BufWriter::new(stdout.lock());

    server
        .serve(stdin.lock(), &mut output)
        .map_err(|err| AppError::Runtime(format!("mcp stdio failed: {err}")))?;

    Ok(())
}
