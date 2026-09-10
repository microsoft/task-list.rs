use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use tasklist_domain::{Task, TaskStatus};

/// Transport representation of a [`Task`]. Timestamps are RFC3339 strings so the shape is
/// stable across the OpenAPI boundary (and the generated TypeScript client).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    /// uuidv7 identifier.
    pub id: String,
    /// Owner (Auth0 subject).
    pub owner: String,
    pub title: String,
    pub status: TaskStatusDto,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// RFC3339 last-update timestamp.
    pub updated_at: String,
}

/// Request body for creating a task. Title-only: status defaults to `Todo` and the id /
/// timestamps are server-minted, so the caller supplies just the title. The domain rule
/// (trimmed, 1..=200, non-blank) is enforced by `Title::parse` in the use-case.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub title: String,
}

/// Transport representation of [`TaskStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatusDto {
    Todo,
    InProgress,
    Done,
}

impl From<TaskStatus> for TaskStatusDto {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Todo => Self::Todo,
            TaskStatus::InProgress => Self::InProgress,
            TaskStatus::Done => Self::Done,
        }
    }
}

impl From<Task> for TaskDto {
    fn from(task: Task) -> Self {
        // Rfc3339 formatting only fails for out-of-range dates, which `OffsetDateTime`
        // cannot represent; `unwrap_or_default` keeps the mapping total.
        let created_at = task.created_at.format(&Rfc3339).unwrap_or_default();
        let updated_at = task.updated_at.format(&Rfc3339).unwrap_or_default();
        Self {
            id: task.id.to_string(),
            owner: task.owner.to_string(),
            title: task.title.to_string(),
            status: task.status.into(),
            created_at,
            updated_at,
        }
    }
}
