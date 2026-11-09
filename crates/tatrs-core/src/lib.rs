//! Format, storage, and validation primitives for `tatrs`.
//!
//! This crate is pure: it never prints, never reads stdin, and never decides an
//! exit code. The CLI is a thin adapter over these types.

pub mod diagnostics;
pub mod huid;
pub mod json;
pub mod props;
pub mod store;
pub mod task;

pub use diagnostics::{Diagnostic, Severity};
pub use huid::{Huid, HuidError};
pub use props::{Properties, Property};
pub use store::{LoadResult, NewTask, Repo, StoreError};
pub use task::Task;
