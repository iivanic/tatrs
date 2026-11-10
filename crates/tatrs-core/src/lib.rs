//! Format, storage, and validation primitives for `tatrs`.
//!
//! This crate is pure: it never prints, never reads stdin, and never decides an
//! exit code. The CLI is a thin adapter over these types.

pub mod claims;
pub mod diagnostics;
pub mod huid;
pub mod json;
pub mod lifecycle;
pub mod props;
pub mod query;
pub mod ready;
pub mod refs;
pub mod store;
pub mod summary;
pub mod syntax;
pub mod task;
pub mod time;
pub mod todo;
pub mod update;

pub use claims::{Claim, ClaimEntry, ClaimError, ClaimStore};
pub use diagnostics::{Diagnostic, Severity};
pub use huid::{Huid, HuidError};
pub use lifecycle::{Context, Counts, Flag, Lifecycle, State};
pub use props::{Properties, Property};
pub use query::{CmpOp, Query};
pub use ready::{Readiness, evaluate};
pub use refs::Reference;
pub use store::{LoadResult, NewTask, Repo, StoreError};
pub use summary::{PriorityCount, Summary, TagCount};
pub use syntax::SyntaxError;
pub use task::{Acceptance, Task};
pub use todo::Todo;
pub use update::{Update, UpdateOp};
