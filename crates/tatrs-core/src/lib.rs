//! Format, storage, and validation primitives for `tatrs`.
//!
//! This crate is pure: it never prints, never reads stdin, and never decides an
//! exit code. The CLI is a thin adapter over these types.

pub mod diagnostics;
pub mod huid;
pub mod json;
pub mod props;
pub mod query;
pub mod refs;
pub mod store;
pub mod summary;
pub mod syntax;
pub mod task;
pub mod todo;
pub mod update;

pub use diagnostics::{Diagnostic, Severity};
pub use huid::{Huid, HuidError};
pub use props::{Properties, Property};
pub use query::{CmpOp, Query};
pub use refs::Reference;
pub use store::{LoadResult, NewTask, Repo, StoreError};
pub use summary::{PriorityCount, Summary, TagCount};
pub use syntax::SyntaxError;
pub use task::Task;
pub use todo::Todo;
pub use update::{Update, UpdateOp};
