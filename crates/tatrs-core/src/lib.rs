//! Format, storage, and validation primitives for `tatrs`.
//!
//! This crate is pure: it never prints, never reads stdin, and never decides an
//! exit code. The CLI is a thin adapter over these types.

pub mod diagnostics;
pub mod huid;
pub mod json;
pub mod props;
pub mod query;
pub mod store;
pub mod syntax;
pub mod task;
pub mod update;

pub use diagnostics::{Diagnostic, Severity};
pub use huid::{Huid, HuidError};
pub use props::{Properties, Property};
pub use query::{CmpOp, Query};
pub use store::{LoadResult, NewTask, Repo, StoreError};
pub use syntax::SyntaxError;
pub use task::Task;
pub use update::{Update, UpdateOp};
