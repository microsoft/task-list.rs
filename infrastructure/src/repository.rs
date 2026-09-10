use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use async_trait::async_trait;

use tasklist_application::error::ApplicationError;
use tasklist_application::ports::TaskRepository;
use tasklist_domain::{ETag, Task, TaskId, UserId};

/// In-memory `TaskRepository` used by default (and by tests) when Cosmos is not
/// configured. It is a faithful double of the Cosmos adapter's optimistic-concurrency
/// contract: `create`/`update` mint a fresh etag and `update` rejects a stale etag with
/// [`ApplicationError::Conflict`], so the 412 → 409 path can be exercised hermetically
/// without the emulator.
#[derive(Default)]
pub struct InMemoryTaskRepository {
    tasks: RwLock<Vec<Task>>,
    etag_seq: AtomicU64,
}

impl InMemoryTaskRepository {
    /// An empty repository.
    pub fn new() -> Self {
        Self::default()
    }

    /// A repository pre-populated with `tasks` (used to seed the skeleton's demo data).
    pub fn seeded(tasks: Vec<Task>) -> Self {
        Self {
            tasks: RwLock::new(tasks),
            etag_seq: AtomicU64::new(0),
        }
    }

    /// Mint a fresh, monotonically-increasing etag (stands in for Cosmos' `_etag`).
    fn next_etag(&self) -> ETag {
        let n = self.etag_seq.fetch_add(1, Ordering::Relaxed);
        ETag::new(format!("etag-{n}"))
    }
}

#[async_trait]
impl TaskRepository for InMemoryTaskRepository {
    async fn list_for_owner(&self, owner: &UserId) -> Result<Vec<Task>, ApplicationError> {
        let guard = self
            .tasks
            .read()
            .map_err(|e| ApplicationError::Repository(e.to_string()))?;
        Ok(guard
            .iter()
            .filter(|t| &t.owner == owner)
            .cloned()
            .collect())
    }

    async fn get(&self, owner: &UserId, id: &TaskId) -> Result<Task, ApplicationError> {
        let guard = self
            .tasks
            .read()
            .map_err(|e| ApplicationError::Repository(e.to_string()))?;
        guard
            .iter()
            .find(|t| &t.owner == owner && &t.id == id)
            .cloned()
            .ok_or(ApplicationError::NotFound)
    }

    async fn create(&self, task: &Task) -> Result<Task, ApplicationError> {
        let mut guard = self
            .tasks
            .write()
            .map_err(|e| ApplicationError::Repository(e.to_string()))?;
        if guard
            .iter()
            .any(|t| t.owner == task.owner && t.id == task.id)
        {
            // Mirrors Cosmos: creating an item whose id already exists returns 409.
            return Err(ApplicationError::Conflict);
        }
        let mut stored = task.clone();
        stored.version = Some(self.next_etag());
        guard.push(stored.clone());
        Ok(stored)
    }

    async fn update(&self, task: &Task) -> Result<Task, ApplicationError> {
        let mut guard = self
            .tasks
            .write()
            .map_err(|e| ApplicationError::Repository(e.to_string()))?;
        let slot = guard
            .iter_mut()
            .find(|t| t.owner == task.owner && t.id == task.id)
            .ok_or(ApplicationError::NotFound)?;
        // Optimistic concurrency: the caller's etag must match the stored one.
        if slot.version != task.version {
            return Err(ApplicationError::Conflict);
        }
        let mut updated = task.clone();
        updated.version = Some(self.next_etag());
        *slot = updated.clone();
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use tasklist_domain::{Task, TaskId, TaskStatus, Title, UserId};

    use super::*;

    fn task_for(owner: &UserId, title: &str) -> Task {
        Task::new(
            TaskId::new(),
            owner.clone(),
            Title::parse(title).unwrap(),
            TaskStatus::Todo,
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn list_for_owner_filters_by_owner() {
        let me = UserId::parse("auth0|me").unwrap();
        let other = UserId::parse("auth0|other").unwrap();
        let repo = InMemoryTaskRepository::seeded(vec![task_for(&me, "a"), task_for(&other, "b")]);

        let mine = repo.list_for_owner(&me).await.unwrap();

        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].owner, me);
    }

    #[tokio::test]
    async fn empty_repository_lists_nothing() {
        let repo = InMemoryTaskRepository::new();
        let owner = UserId::parse("auth0|me").unwrap();
        assert!(repo.list_for_owner(&owner).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_stamps_version_and_get_reads_it_back() {
        let repo = InMemoryTaskRepository::new();
        let me = UserId::parse("auth0|me").unwrap();
        let task = task_for(&me, "buy milk");

        let created = repo.create(&task).await.unwrap();
        assert!(created.version.is_some(), "create must stamp an etag");

        let fetched = repo.get(&me, &task.id).await.unwrap();
        assert_eq!(fetched.version, created.version);
        assert_eq!(fetched.title.as_str(), "buy milk");
    }

    #[tokio::test]
    async fn get_missing_is_not_found() {
        let repo = InMemoryTaskRepository::new();
        let me = UserId::parse("auth0|me").unwrap();
        let err = repo.get(&me, &TaskId::new()).await.unwrap_err();
        assert!(matches!(err, ApplicationError::NotFound));
    }

    #[tokio::test]
    async fn update_with_matching_etag_succeeds_and_rotates_version() {
        let repo = InMemoryTaskRepository::new();
        let me = UserId::parse("auth0|me").unwrap();
        let created = repo.create(&task_for(&me, "draft")).await.unwrap();

        let mut edit = created.clone();
        edit.status = TaskStatus::Done;
        let updated = repo.update(&edit).await.unwrap();

        assert_eq!(updated.status, TaskStatus::Done);
        assert_ne!(
            updated.version, created.version,
            "a successful update rotates the etag"
        );
    }

    #[tokio::test]
    async fn update_with_stale_etag_conflicts() {
        let repo = InMemoryTaskRepository::new();
        let me = UserId::parse("auth0|me").unwrap();
        let created = repo.create(&task_for(&me, "draft")).await.unwrap();

        // First update succeeds and rotates the etag.
        let fresh = repo.update(&created).await.unwrap();
        assert_ne!(fresh.version, created.version);

        // A second update carrying the now-stale original etag must conflict.
        let err = repo.update(&created).await.unwrap_err();
        assert!(matches!(err, ApplicationError::Conflict));
    }

    #[tokio::test]
    async fn update_missing_task_is_not_found() {
        let repo = InMemoryTaskRepository::new();
        let me = UserId::parse("auth0|me").unwrap();
        let err = repo.update(&task_for(&me, "ghost")).await.unwrap_err();
        assert!(matches!(err, ApplicationError::NotFound));
    }

    #[tokio::test]
    async fn create_duplicate_id_conflicts() {
        let repo = InMemoryTaskRepository::new();
        let me = UserId::parse("auth0|me").unwrap();
        let task = task_for(&me, "once");
        repo.create(&task).await.unwrap();
        let err = repo.create(&task).await.unwrap_err();
        assert!(matches!(err, ApplicationError::Conflict));
    }
}
