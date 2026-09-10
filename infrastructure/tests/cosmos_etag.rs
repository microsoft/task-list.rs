//! Integration test proving the **real** Cosmos ETag optimistic-concurrency round-trip
//! against the Cosmos DB Linux emulator.
//!
//! This test is **gated** on the `TASKLIST_COSMOS_EMULATOR` environment variable so a
//! plain `cargo test` stays hermetic (no emulator, no network). Run it with the emulator
//! up:
//!
//! ```text
//! docker compose -f scripts/docker-compose.yml up -d
//! $env:TASKLIST_COSMOS_EMULATOR = "1"
//! cargo test -p tasklist-infrastructure --test cosmos_etag -- --nocapture
//! ```
//!
//! It exercises the full DIP path through the `TaskRepository` port against the concrete
//! `CosmosTaskRepository`: create → point-read → update with a matching etag (succeeds) →
//! update with a stale etag (Cosmos returns 412, surfaced as `ApplicationError::Conflict`).

use time::OffsetDateTime;
use uuid::Uuid;

use tasklist_application::error::ApplicationError;
use tasklist_application::ports::TaskRepository;
use tasklist_domain::{Task, TaskId, TaskStatus, Title, UserId};
use tasklist_infrastructure::{CosmosSettings, CosmosTaskRepository, Environment};

/// The test only runs when explicitly opted in, so unit `cargo test` never needs the
/// emulator or the network.
fn emulator_enabled() -> bool {
    std::env::var("TASKLIST_COSMOS_EMULATOR")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[tokio::test]
async fn etag_round_trip_against_emulator() {
    if !emulator_enabled() {
        eprintln!(
            "skipping cosmos_etag: set TASKLIST_COSMOS_EMULATOR=1 (and start the emulator) to run"
        );
        return;
    }

    let settings = CosmosSettings::from_env();
    let repo = CosmosTaskRepository::connect(&settings, Environment::Local)
        .await
        .expect("connect + provision against the Cosmos emulator");

    // Fresh owner per run so repeated runs never collide in the shared container.
    let owner = UserId::parse(format!("test|{}", Uuid::new_v4())).expect("valid owner");
    let task = Task::new(
        TaskId::new(),
        owner.clone(),
        Title::parse("etag round-trip").expect("valid title"),
        TaskStatus::Todo,
        OffsetDateTime::now_utc(),
    );

    // create → server stamps an etag.
    let created = repo.create(&task).await.expect("create item");
    let created_etag = created
        .version
        .clone()
        .expect("create returns a server etag");

    // point-read by (ownerId, id) → same etag, same data.
    let fetched = repo.get(&owner, &task.id).await.expect("point-read");
    assert_eq!(
        fetched.version, created.version,
        "read returns the create etag"
    );
    assert_eq!(fetched.title.as_str(), "etag round-trip");

    // update carrying the current (matching) etag → succeeds and rotates the etag.
    let mut edit = fetched.clone();
    edit.status = TaskStatus::Done;
    let updated = repo
        .update(&edit)
        .await
        .expect("update with matching etag succeeds");
    assert_eq!(updated.status, TaskStatus::Done);
    let updated_etag = updated.version.clone().expect("update returns a new etag");
    assert_ne!(
        updated_etag, created_etag,
        "a successful update must rotate the etag"
    );

    // update carrying the now-stale original etag → Cosmos 412 → ApplicationError::Conflict.
    let mut stale = created.clone();
    stale.status = TaskStatus::InProgress;
    let err = repo
        .update(&stale)
        .await
        .expect_err("stale etag must be rejected");
    assert!(
        matches!(err, ApplicationError::Conflict),
        "a 412 PreconditionFailed must surface as Conflict, got {err:?}"
    );
}
