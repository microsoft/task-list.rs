use async_trait::async_trait;
use time::OffsetDateTime;

use tasklist_domain::{Task, TaskId, UserId};

use crate::error::ApplicationError;

/// Persistence port for tasks. Implemented by an in-memory fake (default / tests) and by
/// the Cosmos adapter — with no change to callers. Every operation is scoped to a single
/// owner (the Cosmos partition key), so cross-owner access is not expressible.
#[async_trait]
pub trait TaskRepository: Send + Sync {
    /// List all tasks owned by `owner` (a single-partition query).
    async fn list_for_owner(&self, owner: &UserId) -> Result<Vec<Task>, ApplicationError>;

    /// Point-read a single task by `(owner, id)`. Returns [`ApplicationError::NotFound`]
    /// when it does not exist or is not owned by `owner`.
    async fn get(&self, owner: &UserId, id: &TaskId) -> Result<Task, ApplicationError>;

    /// Persist a new task, returning it stamped with the server-issued
    /// [`version`](tasklist_domain::Task::version) etag.
    async fn create(&self, task: &Task) -> Result<Task, ApplicationError>;

    /// Replace an existing task using optimistic concurrency: the store checks the task's
    /// [`version`](tasklist_domain::Task::version) etag via `If-Match`. A stale etag
    /// surfaces as [`ApplicationError::Conflict`] (Cosmos 412 → 409); a missing task
    /// surfaces as [`ApplicationError::NotFound`]. Returns the task with its new etag.
    async fn update(&self, task: &Task) -> Result<Task, ApplicationError>;
}

/// Time port, so use-cases and tests do not read the wall clock directly.
pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}
