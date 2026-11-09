//! Exit-code contract (documented in `docs/PLAN.md` §4).

use std::fmt;

use tatrs_core::StoreError;

#[derive(Debug)]
pub enum AppError {
    /// Bad invocation: wrong flags, invalid HUID argument, read-only violation.
    Usage(String),
    /// Environment or I/O problem.
    Runtime(String),
    /// Validation failed; diagnostics were already printed.
    Invalid,
    /// A task (or match set) was not found.
    NotFound(String),
}

impl AppError {
    pub fn exit_code(&self) -> u8 {
        match self {
            AppError::Runtime(_) => 1,
            AppError::Usage(_) => 2,
            AppError::Invalid => 3,
            AppError::NotFound(_) => 5,
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            AppError::Usage(message) | AppError::Runtime(message) | AppError::NotFound(message) => {
                Some(message)
            }
            AppError::Invalid => None,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message().unwrap_or("invalid task data"))
    }
}

impl std::error::Error for AppError {}

impl From<StoreError> for AppError {
    fn from(err: StoreError) -> Self {
        match &err {
            StoreError::NoRepo { .. } => AppError::Runtime(err.to_string()),
            StoreError::NotFound(_) => AppError::NotFound(err.to_string()),
            _ => AppError::Runtime(err.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Runtime(err.to_string())
    }
}
