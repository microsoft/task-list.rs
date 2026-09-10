//! Task use-cases. Each is a plain `async fn` taking the ports it needs — no mediator.

use crate::dto::{CreateTaskRequest, TaskDto};
use crate::error::ApplicationError;
use crate::ports::{Clock, TaskRepository};
use tasklist_domain::{Task, TaskId, TaskStatus, Title, UserId};

/// List the caller's tasks, mapped to transport DTOs.
///
/// This is the walking-skeleton use-case: it flows handler -> use-case -> `TaskRepository`
/// port -> adapter, proving the dependency-inversion seam end to end.
pub async fn list(
    repo: &dyn TaskRepository,
    owner: &UserId,
) -> Result<Vec<TaskDto>, ApplicationError> {
    let tasks = repo.list_for_owner(owner).await?;
    Ok(tasks.into_iter().map(TaskDto::from).collect())
}

/// Create a task for `owner` from a title-only request, returning the persisted DTO.
///
/// Validates the title (`Title::parse`; invalid → [`ApplicationError::Domain`] → 400),
/// server-mints the id (uuidv7), defaults the status to `Todo`, stamps the timestamps
/// from `clock`, and persists through the port — which returns the task stamped with its
/// server-issued etag.
pub async fn create(
    repo: &dyn TaskRepository,
    clock: &dyn Clock,
    owner: &UserId,
    req: CreateTaskRequest,
) -> Result<TaskDto, ApplicationError> {
    let title = Title::parse(req.title)?;
    let task = Task::new(
        TaskId::new(),
        owner.clone(),
        title,
        TaskStatus::Todo,
        clock.now(),
    );
    let created = repo.create(&task).await?;
    Ok(TaskDto::from(created))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    use async_trait::async_trait;
    use time::OffsetDateTime;

    use tasklist_domain::{ETag, Task, TaskId, TaskStatus, Title, UserId};

    use super::*;
    use crate::dto::TaskStatusDto;

    /// Hand-written in-crate fake of the repository port — proves the use-cases run
    /// against any `TaskRepository` implementation. `create` actually stores and stamps a
    /// fresh etag (so `list`/`get` observe it), mirroring the in-memory adapter's contract
    /// closely enough to exercise the use-cases; `update` echoes its input (etag-conflict
    /// semantics are covered against the in-memory adapter in the `infrastructure` crate).
    #[derive(Default)]
    struct FakeRepo {
        tasks: Mutex<Vec<Task>>,
        etag_seq: AtomicU64,
    }

    impl FakeRepo {
        fn seeded(tasks: Vec<Task>) -> Self {
            Self {
                tasks: Mutex::new(tasks),
                etag_seq: AtomicU64::new(0),
            }
        }

        fn next_etag(&self) -> ETag {
            ETag::new(format!(
                "etag-{}",
                self.etag_seq.fetch_add(1, Ordering::Relaxed)
            ))
        }
    }

    #[async_trait]
    impl TaskRepository for FakeRepo {
        async fn list_for_owner(&self, owner: &UserId) -> Result<Vec<Task>, ApplicationError> {
            Ok(self
                .tasks
                .lock()
                .unwrap()
                .iter()
                .filter(|t| &t.owner == owner)
                .cloned()
                .collect())
        }

        async fn get(&self, owner: &UserId, id: &TaskId) -> Result<Task, ApplicationError> {
            self.tasks
                .lock()
                .unwrap()
                .iter()
                .find(|t| &t.owner == owner && &t.id == id)
                .cloned()
                .ok_or(ApplicationError::NotFound)
        }

        async fn create(&self, task: &Task) -> Result<Task, ApplicationError> {
            let mut stored = task.clone();
            stored.version = Some(self.next_etag());
            self.tasks.lock().unwrap().push(stored.clone());
            Ok(stored)
        }

        async fn update(&self, task: &Task) -> Result<Task, ApplicationError> {
            Ok(task.clone())
        }
    }

    /// Fixed-time `Clock` so the create use-case stamps a deterministic timestamp.
    struct FixedClock(OffsetDateTime);

    impl Clock for FixedClock {
        fn now(&self) -> OffsetDateTime {
            self.0
        }
    }

    fn create_req(title: &str) -> CreateTaskRequest {
        CreateTaskRequest {
            title: title.to_owned(),
        }
    }

    #[tokio::test]
    async fn list_returns_only_the_owners_tasks() {
        let owner = UserId::parse("auth0|me").unwrap();
        let other = UserId::parse("auth0|other").unwrap();
        let now = OffsetDateTime::UNIX_EPOCH;
        let repo = FakeRepo::seeded(vec![
            Task::new(
                TaskId::new(),
                owner.clone(),
                Title::parse("mine").unwrap(),
                TaskStatus::Todo,
                now,
            ),
            Task::new(
                TaskId::new(),
                other,
                Title::parse("theirs").unwrap(),
                TaskStatus::Done,
                now,
            ),
        ]);

        let result = list(&repo, &owner).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "mine");
        assert_eq!(result[0].owner, "auth0|me");
    }

    #[tokio::test]
    async fn list_is_empty_when_owner_has_no_tasks() {
        let repo = FakeRepo::default();
        let owner = UserId::parse("auth0|nobody").unwrap();
        assert!(list(&repo, &owner).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_persists_task_and_it_appears_in_list() {
        let repo = FakeRepo::default();
        let clock = FixedClock(OffsetDateTime::UNIX_EPOCH);
        let owner = UserId::parse("auth0|me").unwrap();

        let created = create(&repo, &clock, &owner, create_req("  Buy milk  "))
            .await
            .unwrap();

        // Title is trimmed by the domain and status defaults to Todo.
        assert_eq!(created.title, "Buy milk");
        assert_eq!(created.status, TaskStatusDto::Todo);
        assert!(!created.id.is_empty(), "id is server-minted");

        let listed = list(&repo, &owner).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].title, "Buy milk");
    }

    #[tokio::test]
    async fn create_defaults_status_to_todo() {
        let repo = FakeRepo::default();
        let clock = FixedClock(OffsetDateTime::UNIX_EPOCH);
        let owner = UserId::parse("auth0|me").unwrap();

        let created = create(&repo, &clock, &owner, create_req("task"))
            .await
            .unwrap();

        assert_eq!(created.status, TaskStatusDto::Todo);
    }

    #[tokio::test]
    async fn create_rejects_blank_title() {
        let repo = FakeRepo::default();
        let clock = FixedClock(OffsetDateTime::UNIX_EPOCH);
        let owner = UserId::parse("auth0|me").unwrap();

        let err = create(&repo, &clock, &owner, create_req("   "))
            .await
            .unwrap_err();

        assert!(matches!(err, ApplicationError::Domain(_)));
        // Nothing was persisted.
        assert!(list(&repo, &owner).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_rejects_oversize_title() {
        let repo = FakeRepo::default();
        let clock = FixedClock(OffsetDateTime::UNIX_EPOCH);
        let owner = UserId::parse("auth0|me").unwrap();

        let err = create(&repo, &clock, &owner, create_req(&"a".repeat(201)))
            .await
            .unwrap_err();

        assert!(matches!(err, ApplicationError::Domain(_)));
    }

    #[tokio::test]
    async fn create_is_owner_scoped() {
        let repo = FakeRepo::default();
        let clock = FixedClock(OffsetDateTime::UNIX_EPOCH);
        let owner_a = UserId::parse("auth0|a").unwrap();
        let owner_b = UserId::parse("auth0|b").unwrap();

        create(&repo, &clock, &owner_a, create_req("a's task"))
            .await
            .unwrap();

        // Visible to A, invisible to B.
        assert_eq!(list(&repo, &owner_a).await.unwrap().len(), 1);
        assert!(list(&repo, &owner_b).await.unwrap().is_empty());
    }
}
