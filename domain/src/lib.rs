//! Domain layer: entities, value-object newtypes, enums and errors.
//!
//! This crate has **zero inbound dependencies** and only leaf crates (`uuid`, `time`,
//! `thiserror`). Invariants are enforced at construction (parse-don't-validate) so an
//! invalid `Task`, `Title` or `UserId` is unrepresentable.

mod error;
mod ids;
mod task;

pub use error::DomainError;
pub use ids::{ETag, TaskId, Title, UserId};
pub use task::{Task, TaskStatus};
